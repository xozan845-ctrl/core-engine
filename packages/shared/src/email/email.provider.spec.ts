import * as nodemailer from 'nodemailer';
import { EmailModule } from './email.module';
import { LogEmailProvider, SmtpEmailProvider, crearEmailProvider, maskEmail } from './email.provider';

vi.mock('nodemailer', () => ({ createTransport: vi.fn() }));

describe('email provider', () => {
  beforeEach(() => {
    vi.mocked(nodemailer.createTransport).mockReset();
  });

  it('enmascara el correo sin exponer el local completo', () => {
    expect(maskEmail('ana@core.test')).toBe('a***@core.test');
    expect(maskEmail('ab@x')).toBe('a***@x');
    expect(maskEmail('sin-arroba')).toBe('***');
    expect(maskEmail('@core.test')).toBe('***');
    expect(maskEmail('ana@')).toBe('***');
  });

  it('usa log por defecto y no registra el cuerpo ni el token', async () => {
    const spy = vi.spyOn(console, 'log').mockImplementation(() => undefined);
    await new LogEmailProvider().send({
      to: 'ana@core.test',
      subject: 'Hola',
      text: 'token-secreto-123',
    });
    const salida = spy.mock.calls.map((c) => String(c[0])).join('\n');
    expect(salida).toContain('a***@core.test');
    expect(salida).not.toContain('token-secreto-123');

    expect(crearEmailProvider({})).toBeInstanceOf(LogEmailProvider);
    spy.mockRestore();
  });

  it('crea el proveedor SMTP con TLS para el puerto 465', () => {
    vi.mocked(nodemailer.createTransport).mockReturnValue({ sendMail: vi.fn() } as never);
    const provider = crearEmailProvider({
      EMAIL_MODE: 'smtp',
      SMTP_HOST: 'smtp.test',
      SMTP_PORT: '465',
      EMAIL_FROM: 'no-reply@test',
    });

    expect(provider).toBeInstanceOf(SmtpEmailProvider);
    expect(nodemailer.createTransport).toHaveBeenCalledWith({
      host: 'smtp.test',
      port: 465,
      secure: true,
    });
  });

  it('envía por SMTP con auth y html opcional', async () => {
    const sendMail = vi.fn();
    vi.mocked(nodemailer.createTransport).mockReturnValue({ sendMail } as never);
    const provider = new SmtpEmailProvider({
      host: 'smtp.test',
      port: 587,
      secure: false,
      from: 'no-reply@test',
      user: 'usuario',
      pass: 'secreto',
    });

    await provider.send({
      to: 'ana@test',
      subject: 'Hola',
      text: 'cuerpo',
      html: '<b>cuerpo</b>',
    });

    expect(nodemailer.createTransport).toHaveBeenCalledWith({
      host: 'smtp.test',
      port: 587,
      secure: false,
      auth: { user: 'usuario', pass: 'secreto' },
    });
    expect(sendMail).toHaveBeenCalledWith({
      from: 'no-reply@test',
      to: 'ana@test',
      subject: 'Hola',
      text: 'cuerpo',
      html: '<b>cuerpo</b>',
    });
  });

  it('rechaza SMTP incompleto, credenciales a medias y modos inválidos', () => {
    expect(() => crearEmailProvider({ EMAIL_MODE: 'smtp' })).toThrow(/SMTP_HOST/);
    expect(() =>
      crearEmailProvider({
        EMAIL_MODE: 'smtp',
        SMTP_HOST: 'smtp.test',
        SMTP_PORT: '587',
        EMAIL_FROM: 'no-reply@test',
        SMTP_USER: 'usuario',
      }),
    ).toThrow(/SMTP_USER/);
    expect(() => crearEmailProvider({ EMAIL_MODE: 'otro' })).toThrow(/EMAIL_MODE/);
  });

  it('expone el módulo Nest de email', () => {
    expect(EmailModule).toBeDefined();
  });
});
