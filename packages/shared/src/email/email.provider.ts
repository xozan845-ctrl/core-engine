import * as nodemailer from 'nodemailer';
import { Logger } from '../logging/logger';
import type { EmailMessage, EmailMode, EmailProvider } from './email.types';

export const EMAIL_PROVIDER = 'EMAIL_PROVIDER';

/** Enmascara el correo para logs: conserva la primera letra y el dominio. */
export function maskEmail(correo: string): string {
  const arroba = correo.indexOf('@');
  if (arroba <= 0 || arroba === correo.length - 1) return '***';
  return `${correo.slice(0, 1)}***@${correo.slice(arroba + 1)}`;
}

/**
 * Modo log: no envía correo. Deja una traza sin cuerpo ni token para que
 * dev/test puedan validar el flujo sin SMTP ni datos sensibles.
 */
export class LogEmailProvider implements EmailProvider {
  private readonly logger = Logger.create('email.log');

  async send(message: EmailMessage): Promise<void> {
    this.logger.info({
      msg: 'email_simulado',
      to: maskEmail(message.to),
      subject: message.subject,
    });
  }
}

export interface SmtpEmailConfig {
  host: string;
  port: number;
  secure: boolean;
  from: string;
  user?: string;
  pass?: string;
}

/** SMTP genérico (Supabase Auth SMTP, relay corporativo, etc.). */
export class SmtpEmailProvider implements EmailProvider {
  private readonly transporter: ReturnType<typeof nodemailer.createTransport>;

  constructor(private readonly config: SmtpEmailConfig) {
    this.transporter = nodemailer.createTransport({
      host: config.host,
      port: config.port,
      secure: config.secure,
      ...(config.user ? { auth: { user: config.user, pass: config.pass } } : {}),
    });
  }

  async send(message: EmailMessage): Promise<void> {
    await this.transporter.sendMail({
      from: this.config.from,
      to: message.to,
      subject: message.subject,
      text: message.text,
      ...(message.html ? { html: message.html } : {}),
    });
  }
}

function leerModo(valor: string | undefined): EmailMode {
  const modo = (valor ?? 'log').trim().toLowerCase();
  if (modo !== 'log' && modo !== 'smtp') {
    throw new Error(`EMAIL_MODE invalido: "${modo}". Usa log o smtp.`);
  }
  return modo;
}

/**
 * Fábrica única: `log` por defecto; `smtp` exige configuración completa y
 * falla al arrancar si falta (R-DS-1: los secretos vienen del entorno).
 */
export function crearEmailProvider(env: NodeJS.ProcessEnv = process.env): EmailProvider {
  const modo = leerModo(env.EMAIL_MODE);
  if (modo === 'log') return new LogEmailProvider();

  const host = env.SMTP_HOST?.trim();
  const from = env.EMAIL_FROM?.trim();
  const puerto = Number(env.SMTP_PORT ?? 587);
  if (!host || !from || !Number.isInteger(puerto) || puerto < 1 || puerto > 65535) {
    throw new Error('Con EMAIL_MODE=smtp son obligatorios SMTP_HOST, SMTP_PORT y EMAIL_FROM.');
  }
  if (Boolean(env.SMTP_USER) !== Boolean(env.SMTP_PASSWORD)) {
    throw new Error('SMTP_USER y SMTP_PASSWORD deben configurarse juntos o ninguno.');
  }

  return new SmtpEmailProvider({
    host,
    port: puerto,
    secure: puerto === 465,
    from,
    ...(env.SMTP_USER ? { user: env.SMTP_USER, pass: env.SMTP_PASSWORD } : {}),
  });
}
