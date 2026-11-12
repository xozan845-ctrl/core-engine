import { createHash } from 'crypto';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { secretoJwt, verificarToken, TokenPayload, ROLES } from '@core/shared';
import { ApiKeysService } from './api-keys.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const USUARIO = '22222222-2222-4222-8222-222222222222';
const KEY_ID = '33333333-3333-4333-8333-333333333333';

function sha256(valor: string): string {
  return createHash('sha256').update(valor, 'utf8').digest('hex');
}

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn(), transaccion: vi.fn() };
  const service = new ApiKeysService(pg as never);
  return { pg, service };
}

const empresario = {
  user_id: USUARIO,
  email: 'ana@test.dev',
  rol: 'empresario',
  tenant_id: EMPRESA,
} as never;

const admin = {
  user_id: USUARIO,
  email: 'admin@test.dev',
  rol: 'admin',
} as never;

function filaKey(extra: Partial<Record<string, unknown>> = {}) {
  return {
    id: KEY_ID,
    empresa_id: EMPRESA,
    nombre: 'Integración contable',
    prefijo: 'ab12cd34',
    rol: ROLES.VENDEDOR,
    estado: 'activa',
    expira_en: null,
    creado_en: '2026-11-12T00:00:00.000Z',
    ultimo_uso_en: null,
    ...extra,
  };
}

describe('ApiKeysService (ADR-0005/R-GW-8)', () => {
  afterEach(() => vi.unstubAllEnvs());

  it('debe generar una key mostrada una vez y guardar solo su hash', async () => {
    const { pg, service } = crear();
    pg.queryOne.mockResolvedValue(filaKey());

    const creada = await service.crear(empresario, {
      nombre: ' Integración contable ',
      rol: ROLES.VENDEDOR,
    });

    expect(creada.key).toMatch(/^cek_[0-9a-f]{8}_[A-Za-z0-9_-]{40,}$/);
    const [sql, params] = pg.queryOne.mock.calls[0];
    expect(sql).toContain('INSERT INTO identity.api_keys');
    expect(params[2]).toBe('Integración contable');
    expect(params[3]).toBe(creada.key.split('_')[1]);
    expect(params[4]).toBe(sha256(creada.key));
    expect(params[5]).toBe(ROLES.VENDEDOR);
    expect(params[7]).toBe(USUARIO);
  });

  it('debe rechazar roles administrativos en una key', async () => {
    const { pg, service } = crear();
    await expect(
      service.crear(empresario, { nombre: 'root', rol: ROLES.SUPER_ADMIN }),
    ).rejects.toMatchObject({ codigo: 'ROL_NO_PERMITIDO' });
    expect(pg.queryOne).not.toHaveBeenCalled();
  });

  it('debe exigir empresa al admin y tenant al empresario', async () => {
    const { service } = crear();
    await expect(
      service.crear(admin, { nombre: 'api', rol: ROLES.VENDEDOR }),
    ).rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
    await expect(
      service.crear({ user_id: USUARIO, rol: ROLES.EMPRESARIO } as never, {
        nombre: 'api',
        rol: ROLES.VENDEDOR,
      }),
    ).rejects.toMatchObject({ codigo: 'TENANT_REQUERIDO' });
  });

  it('debe listar solo el tenant del empresario y permitir empresa al admin', async () => {
    const { pg, service } = crear();
    pg.query.mockResolvedValue([]);

    await service.listar(empresario);
    expect(pg.query.mock.calls[0][1]).toEqual([EMPRESA]);

    await service.listar(admin, '44444444-4444-4444-8444-444444444444');
    expect(pg.query.mock.calls[1][1]).toEqual(['44444444-4444-4444-8444-444444444444']);
  });

  it('debe devolver 404 al revocar una key inexistente o de otro tenant', async () => {
    const { pg, service } = crear();
    pg.queryOne.mockResolvedValue(null);
    await expect(service.revocar(empresario, KEY_ID)).rejects.toMatchObject({
      codigo: 'NO_ENCONTRADO',
    });
    expect(pg.queryOne.mock.calls[0][1]).toEqual([KEY_ID, EMPRESA]);
  });

  it('debe intercambiar una key activa por un JWT de acceso corto', async () => {
    vi.stubEnv('JWT_SECRET', 'secreto-de-prueba');
    vi.stubEnv('JWT_ACCESS_TTL', '900s');
    const { pg, service } = crear();
    pg.queryOne.mockResolvedValue(filaKey());
    pg.query.mockResolvedValue([]);

    const token = await service.token('cek_ab12cd34_secreto');
    expect(token.token_type).toBe('Bearer');
    expect(token.expires_in).toBe(900);

    const payload = verificarToken<TokenPayload>(token.access_token, secretoJwt());
    expect(payload.tipo).toBe('access');
    expect(payload.sub).toBe(`api-key:${KEY_ID}`);
    expect(payload.rol).toBe(ROLES.VENDEDOR);
    expect(payload.tenant_id).toBe(EMPRESA);
    expect(pg.query.mock.calls[0][0]).toContain('ultimo_uso_en = NOW()');
  });

  it('debe rechazar keys ausentes, revocadas o expiradas', async () => {
    const { pg, service } = crear();
    await expect(service.token(undefined)).rejects.toMatchObject({ codigo: 'NO_AUTORIZADO' });
    await expect(service.token('otra_cosa')).rejects.toMatchObject({ codigo: 'NO_AUTORIZADO' });

    pg.queryOne.mockResolvedValue(null);
    await expect(service.token('cek_ab12cd34_x')).rejects.toMatchObject({
      codigo: 'NO_AUTORIZADO',
    });

    pg.queryOne.mockResolvedValue(filaKey({ expira_en: '2020-01-01T00:00:00.000Z' }));
    await expect(service.token('cek_ab12cd34_x')).rejects.toMatchObject({
      codigo: 'NO_AUTORIZADO',
    });
  });
});
