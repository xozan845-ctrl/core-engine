import * as bcrypt from 'bcryptjs';
import { createHash } from 'node:crypto';
import type { EmailProvider, PgService } from '@core/shared';
import { DomainError, GoneError } from '@core/shared';
import type { AuthService } from './auth.service';
import { PasswordResetService } from './password-reset.service';
import type { UsuariosService } from '../usuarios/usuarios.service';

const USUARIO = {
  id: 'u-1',
  nombre: 'Ana',
  correo: 'ana@core.test',
  rol: 'comprador' as const,
  contrasena_hash: 'hash-viejo',
  creado_en: '2026-09-01T00:00:00.000Z',
};

function hash(token: string): string {
  return createHash('sha256').update(token).digest('hex');
}

function crear(filaInicial: Record<string, unknown> | null = null) {
  const cliente = { query: vi.fn().mockResolvedValue({ rows: [], rowCount: 1 }) };
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(filaInicial),
    transaccion: vi.fn(async (fn: (c: typeof cliente) => Promise<unknown>) => fn(cliente)),
  };
  const usuarios = {
    encontrarPorCorreo: vi.fn().mockResolvedValue(USUARIO),
  };
  const auth = { cerrarSesion: vi.fn().mockResolvedValue(1) };
  const email = { send: vi.fn().mockResolvedValue(undefined) };
  const servicio = new PasswordResetService(
    pg as unknown as PgService,
    usuarios as unknown as UsuariosService,
    auth as unknown as AuthService,
    email as unknown as EmailProvider,
  );
  return { servicio, pg, usuarios, auth, email, cliente };
}

describe('PasswordResetService (ADR-0003)', () => {
  it('no revela ni escribe nada si el correo no existe', async () => {
    const { servicio, pg, usuarios, email } = crear();
    usuarios.encontrarPorCorreo.mockResolvedValue(null);

    await expect(servicio.solicitar('nadie@core.test')).resolves.toBeUndefined();
    expect(pg.transaccion).not.toHaveBeenCalled();
    expect(email.send).not.toHaveBeenCalled();
  });

  it('invalida tokens previos, guarda solo el hash y envía el enlace', async () => {
    const { servicio, cliente, email } = crear();

    await servicio.solicitar('ana@core.test');

    const llamadas = cliente.query.mock.calls.map(([sql]) => String(sql));
    expect(llamadas[0]).toContain('SET usado_en=NOW()');
    expect(llamadas[1]).toContain('INSERT INTO identity.password_reset_tokens');

    const [, hashGuardado, expira] = cliente.query.mock.calls[1][1] as [string, string, Date];
    const mensaje = email.send.mock.calls[0][0] as { to: string; text: string };
    const token = /token=([^\s&]+)/.exec(mensaje.text)![1];
    expect(mensaje.to).toBe('ana@core.test');
    expect(hashGuardado).toBe(hash(token));
    expect(hashGuardado).not.toContain(token);
    const ttl = new Date(expira).getTime() - Date.now();
    expect(ttl).toBeGreaterThan(29 * 60 * 1000);
    expect(ttl).toBeLessThanOrEqual(30 * 60 * 1000);
  });

  it('si el correo falla no rompe la respuesta e invalida el token', async () => {
    const { servicio, pg, email } = crear();
    email.send.mockRejectedValue(new Error('SMTP caido'));
    const spy = vi.spyOn(console, 'error').mockImplementation(() => undefined);

    await expect(servicio.solicitar('ana@core.test')).resolves.toBeUndefined();

    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('SET usado_en=NOW()'),
      [expect.any(String)],
    );
    spy.mockRestore();
  });

  it('rechaza una contraseña corta antes de tocar la base de datos', async () => {
    const { servicio, pg } = crear();

    await expect(servicio.confirmar('token', 'corta')).rejects.toMatchObject({
      codigo: 'CONTRASENA_DEBIL',
    });
    expect(pg.queryOne).not.toHaveBeenCalled();
  });

  it.each([
    [null, 'TOKEN_INVALIDO'],
    [{ usuario_id: 'u-1', expira_en: new Date(Date.now() + 60_000), usado_en: new Date() }, 'TOKEN_USADO'],
    [{ usuario_id: 'u-1', expira_en: new Date(Date.now() - 1), usado_en: null }, 'TOKEN_EXPIRADO'],
  ])('rechaza un token no vigente (%s)', async (fila, codigo) => {
    const { servicio } = crear(fila as Record<string, unknown> | null);
    await expect(servicio.confirmar('token', 'NuevaClave123!')).rejects.toMatchObject({ codigo });
  });

  it('cambia la contraseña, consume los tokens y revoca las sesiones', async () => {
    const fila = { usuario_id: 'u-1', expira_en: new Date(Date.now() + 60_000), usado_en: null };
    const { servicio, cliente, auth } = crear(fila);
    cliente.query.mockResolvedValueOnce({ rows: [fila], rowCount: 1 });

    await servicio.confirmar('token-valido', 'NuevaClave123!');

    const sentencias = cliente.query.mock.calls.map(([sql]) => String(sql));
    expect(sentencias[0]).toContain('FOR UPDATE');
    expect(sentencias[1]).toContain('UPDATE identity.usuarios SET contrasena_hash');
    expect(sentencias[2]).toContain('SET usado_en=NOW()');

    const [hashNuevo] = cliente.query.mock.calls[1][1] as [string];
    await expect(bcrypt.compare('NuevaClave123!', hashNuevo)).resolves.toBe(true);
    expect(auth.cerrarSesion).toHaveBeenCalledWith('u-1');
  });

  it('detecta la carrera si otro proceso consumió el token en la transacción', async () => {
    const fila = { usuario_id: 'u-1', expira_en: new Date(Date.now() + 60_000), usado_en: null };
    const { servicio, cliente, auth } = crear(fila);
    cliente.query.mockResolvedValueOnce({ rows: [], rowCount: 0 });

    await expect(servicio.confirmar('token-valido', 'NuevaClave123!')).rejects.toBeInstanceOf(GoneError);
    expect(auth.cerrarSesion).not.toHaveBeenCalled();
  });

  it('mantiene el código de error de dominio en contraseña débil', async () => {
    const { servicio } = crear();
    await expect(servicio.confirmar('token', 'corta')).rejects.toBeInstanceOf(DomainError);
  });
});
