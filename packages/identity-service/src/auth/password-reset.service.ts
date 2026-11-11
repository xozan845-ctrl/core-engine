import { Inject, Injectable } from '@nestjs/common';
import * as bcrypt from 'bcryptjs';
import { createHash, randomBytes } from 'node:crypto';
import {
  DomainError,
  EMAIL_PROVIDER,
  EmailProvider,
  GoneError,
  Logger,
  PgService,
} from '@core/shared';
import { AuthService } from './auth.service';
import { UsuariosService } from '../usuarios/usuarios.service';

/** Vigencia del enlace de restablecimiento (ADR-0003). */
const TOKEN_TTL_MS = 30 * 60 * 1000;
const BCRYPT_COST = 10;

interface FilaToken {
  usuario_id: string;
  expira_en: string | Date;
  usado_en: string | Date | null;
}

function hashToken(token: string): string {
  return createHash('sha256').update(token).digest('hex');
}

function expirado(fila: FilaToken): boolean {
  return new Date(fila.expira_en).getTime() <= Date.now();
}

/**
 * Restablecimiento de contraseña (ADR-0003, fase 2).
 *
 * - El token en claro solo viaja en el correo; en BD queda su hash SHA-256.
 * - Un solo uso y 30 minutos de vigencia; solicitar uno nuevo invalida los
 *   anteriores del mismo usuario.
 * - La respuesta de la solicitud nunca revela si el correo existe (OWASP A07).
 */
@Injectable()
export class PasswordResetService {
  private readonly frontendUrl = (process.env.FRONTEND_URL ?? 'http://localhost:5173').replace(/\/+$/, '');
  private readonly logger = Logger.create('identity.password-reset');

  constructor(
    private readonly pg: PgService,
    private readonly usuarios: UsuariosService,
    private readonly auth: AuthService,
    @Inject(EMAIL_PROVIDER) private readonly email: EmailProvider,
  ) {}

  async solicitar(correo: string): Promise<void> {
    const usuario = await this.usuarios.encontrarPorCorreo(correo);
    if (!usuario) return;

    const token = randomBytes(32).toString('base64url');
    const tokenHash = hashToken(token);
    const expiraEn = new Date(Date.now() + TOKEN_TTL_MS);
    await this.pg.transaccion(async (client) => {
      await client.query(
        `UPDATE identity.password_reset_tokens SET usado_en=NOW()
          WHERE usuario_id=$1 AND usado_en IS NULL`,
        [usuario.id],
      );
      await client.query(
        `INSERT INTO identity.password_reset_tokens (usuario_id, token_hash, expira_en)
         VALUES ($1,$2,$3)`,
        [usuario.id, tokenHash, expiraEn],
      );
    });

    const enlace = `${this.frontendUrl}/restablecer-contrasena?token=${encodeURIComponent(token)}`;
    try {
      await this.email.send({
        to: usuario.correo,
        subject: 'Restablece tu contraseña',
        text: `Recibimos una solicitud para restablecer tu contraseña. Abre este enlace (válido 30 minutos): ${enlace}\n\nSi no fuiste tú, ignora este correo.`,
        html: `<p>Recibimos una solicitud para restablecer tu contraseña.</p><p><a href="${enlace}">Restablecer contraseña</a> (válido 30 minutos).</p><p>Si no fuiste tú, ignora este correo.</p>`,
      });
    } catch (error) {
      // No se propaga: la respuesta debe ser idéntica exista o no la cuenta.
      // Se invalida el token para no dejar un enlace que nunca se entregó.
      // Solo se registra el código SMTP: nunca el correo ni el token (R-OB-1).
      const codigo = (error as { code?: unknown })?.code;
      this.logger.error({
        msg: 'email_reset_fallido',
        ...(typeof codigo === 'string' ? { codigo } : {}),
      });
      await this.pg
        .query(`UPDATE identity.password_reset_tokens SET usado_en=NOW() WHERE token_hash=$1`, [tokenHash])
        .catch(() => undefined);
    }
  }

  async confirmar(token: string, nueva: string): Promise<void> {
    if (nueva.length < 8) {
      throw new DomainError('CONTRASENA_DEBIL', 'La nueva contrasena debe tener al menos 8 caracteres.');
    }
    const tokenHash = hashToken(token.trim());
    const fila = await this.pg.queryOne<FilaToken>(
      `SELECT usuario_id, expira_en, usado_en FROM identity.password_reset_tokens WHERE token_hash=$1`,
      [tokenHash],
    );
    this.validarToken(fila);

    const hash = await bcrypt.hash(nueva, BCRYPT_COST);
    const usuarioId = await this.pg.transaccion(async (client) => {
      const resultado = await client.query<FilaToken>(
        `SELECT usuario_id, expira_en, usado_en FROM identity.password_reset_tokens
          WHERE token_hash=$1 FOR UPDATE`,
        [tokenHash],
      );
      const vigente = resultado.rows[0] ?? null;
      this.validarToken(vigente);
      const id = vigente!.usuario_id;
      await client.query(`UPDATE identity.usuarios SET contrasena_hash=$1 WHERE id=$2`, [hash, id]);
      // Consume este token y cualquier otro pendiente del usuario.
      await client.query(
        `UPDATE identity.password_reset_tokens SET usado_en=NOW()
          WHERE usuario_id=$1 AND usado_en IS NULL`,
        [id],
      );
      return id;
    });

    // Cambiar la contraseña invalida las sesiones abiertas (R-GW-4, ADR-18).
    await this.auth.cerrarSesion(usuarioId);
  }

  private validarToken(fila: FilaToken | null | undefined): asserts fila is FilaToken {
    if (!fila) throw new GoneError('TOKEN_INVALIDO', 'El enlace no es válido.');
    if (fila.usado_en) throw new GoneError('TOKEN_USADO', 'El enlace ya fue utilizado.');
    if (expirado(fila)) throw new GoneError('TOKEN_EXPIRADO', 'El enlace ha expirado.');
  }
}
