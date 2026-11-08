import { describe, expect, it, vi } from 'vitest';
import { ForbiddenError, NotFoundError } from '@core/shared';
import { PartesService } from './partes.service';

const EMPRESA = '11111111-2222-4333-8444-555555555555';
const PARTE = '22222222-3333-4444-8555-666666666666';
const USUARIO = '44444444-5555-4666-8777-888888888888';
const actor = { user_id: USUARIO, rol: 'empresario' as const, tenant_id: EMPRESA };

function crearPg(overrides: Record<string, unknown> = {}) {
  return {
    query: vi.fn(),
    queryOne: vi.fn().mockResolvedValue({ 1: 1 }),
    transaccion: vi.fn(),
    ...overrides,
  };
}

describe('PartesService', () => {
  it('crea una parte con valores normalizados y metadata', async () => {
    const fila = { id: PARTE, empresa_id: EMPRESA, tipo: 'persona', nombre: 'Ana María', metadata: { origen: 'import' } };
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce(fila) });
    const servicio = new PartesService(pg as never);

    const resultado = await servicio.crear(EMPRESA, { tipo: 'persona', nombre: '  Ana María  ', metadata: { origen: 'import' } }, actor);

    expect(resultado).toEqual(fila);
    expect(pg.queryOne).toHaveBeenLastCalledWith(expect.stringContaining('INSERT INTO identity.partes'), [
      EMPRESA, 'persona', 'Ana María', '', '', '', 'activo', '{"origen":"import"}', USUARIO,
    ]);
  });

  it('rechaza operar sobre otro tenant', async () => {
    const pg = crearPg();
    const servicio = new PartesService(pg as never);

    await expect(servicio.listar('55555555-6666-4777-8888-999999999999', {}, actor)).rejects.toBeInstanceOf(ForbiddenError);
    expect(pg.queryOne).not.toHaveBeenCalled();
  });

  it('lista con filtros parametrizados y límite acotado', async () => {
    const pg = crearPg({ query: vi.fn().mockResolvedValue([]) });
    const servicio = new PartesService(pg as never);

    await servicio.listar(EMPRESA, { q: 'ana', tipo: 'persona', limite: 500 }, actor);

    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('ILIKE'), [EMPRESA, 'persona', '%ana%', 100]);
  });

  it('crea un contacto solo después de validar la parte del tenant', async () => {
    const parte = { id: PARTE, empresa_id: EMPRESA, eliminado_en: null };
    const contacto = { id: '66666666-7777-4888-8999-aaaaaaaaaaaa', empresa_id: EMPRESA, parte_id: PARTE, tipo: 'email', valor: 'ana@example.test' };
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce(parte).mockResolvedValueOnce(contacto) });
    const servicio = new PartesService(pg as never);

    await expect(servicio.crearContacto(EMPRESA, PARTE, { tipo: 'email', valor: 'ana@example.test' }, actor)).resolves.toEqual(contacto);
    expect(pg.queryOne).toHaveBeenLastCalledWith(expect.stringContaining('INSERT INTO identity.medios_contacto'), expect.any(Array));
  });

  it('valida coordenadas y metadata antes de persistir', async () => {
    const pg = crearPg();
    const servicio = new PartesService(pg as never);

    await expect(servicio.crearDireccion(EMPRESA, PARTE, {
      tipo: 'entrega', linea_1: 'Calle 1', latitud: 91,
    }, actor)).rejects.toMatchObject({ codigo: 'COORDENADA_INVALIDA' });
    await expect(servicio.crear(EMPRESA, {
      tipo: 'organizacion', nombre: 'Acme', metadata: [] as never,
    }, actor)).rejects.toMatchObject({ codigo: 'METADATA_INVALIDA' });
  });

  it('impide relaciones consigo misma y exige el destino en el mismo tenant', async () => {
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE, empresa_id: EMPRESA, eliminado_en: null }) });
    const servicio = new PartesService(pg as never);

    await expect(servicio.crearRelacion(EMPRESA, PARTE, PARTE, 'cliente', undefined, actor)).rejects.toMatchObject({ codigo: 'RELACION_INVALIDA' });
    expect(pg.queryOne).toHaveBeenCalledTimes(2);
  });

  it('archiva una parte y sus datos derivados en una transacción', async () => {
    const client = { query: vi.fn().mockResolvedValue({ rowCount: 1, rows: [{ id: PARTE }] }) };
    const pg = crearPg({ transaccion: vi.fn(async (fn: (client: typeof client) => Promise<unknown>) => fn(client)) });
    const servicio = new PartesService(pg as never);

    await expect(servicio.eliminar(EMPRESA, PARTE, actor)).resolves.toEqual({ ok: true });
    expect(client.query).toHaveBeenCalledTimes(4);
    expect(client.query.mock.calls[1][0]).toContain('medios_contacto');
    expect(client.query.mock.calls[2][0]).toContain('direcciones_partes');
    expect(client.query.mock.calls[3][0]).toContain('relaciones_partes');
  });

  it('devuelve 404 cuando la parte a archivar no existe', async () => {
    const client = { query: vi.fn().mockResolvedValue({ rowCount: 0, rows: [] }) };
    const pg = crearPg({ transaccion: vi.fn(async (fn: (client: typeof client) => Promise<unknown>) => fn(client)) });
    const servicio = new PartesService(pg as never);

    await expect(servicio.eliminar(EMPRESA, PARTE, actor)).rejects.toBeInstanceOf(NotFoundError);
    expect(client.query).toHaveBeenCalledTimes(1);
  });
});
