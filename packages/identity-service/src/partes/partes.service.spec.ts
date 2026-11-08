import { describe, expect, it, vi } from 'vitest';
import { ForbiddenError, NotFoundError } from '@core/shared';
import { PartesService } from './partes.service';

const EMPRESA = '11111111-2222-4333-8444-555555555555';
const PARTE = '22222222-3333-4444-8555-666666666666';
const USUARIO = '44444444-5555-4666-8777-888888888888';
const actor = { user_id: USUARIO, rol: 'empresario' as const, tenant_id: EMPRESA };
const admin = { user_id: USUARIO, rol: 'admin' as const };
const parteFila = { id: PARTE, empresa_id: EMPRESA, tipo: 'persona', nombre: 'Ana', eliminado_en: null };
const contactoFila = { id: '66666666-7777-4888-8999-aaaaaaaaaaaa', empresa_id: EMPRESA, parte_id: PARTE, tipo: 'email', valor: 'ana@example.test' };
const direccionFila = { id: '77777777-8888-4999-8aaa-bbbbbbbbbbbb', empresa_id: EMPRESA, parte_id: PARTE, tipo: 'entrega', linea_1: 'Calle 1' };
const relacionFila = { id: '88888888-9999-4aaa-8bbb-cccccccccccc', empresa_id: EMPRESA, parte_origen_id: PARTE, parte_destino_id: '99999999-aaaa-4bbb-8ccc-dddddddddddd', tipo: 'cliente' };

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

  it('obtiene y actualiza una parte, traduciendo ausencia y unicidad', async () => {
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValue(parteFila) });
    const servicio = new PartesService(pg as never);

    await expect(servicio.obtener(EMPRESA, PARTE, admin)).resolves.toEqual(parteFila);
    await expect(servicio.actualizar(EMPRESA, PARTE, {
      tipo: 'organizacion', nombre: '  Acme  ', nombre_comercial: 'Acme S.A.',
      identificador_tipo: 'ruc', identificador_valor: 'J-1', estado: 'inactivo', metadata: { origen: 'test' },
    }, admin)).resolves.toEqual(parteFila);

    pg.queryOne.mockResolvedValueOnce(null);
    await expect(servicio.actualizar(EMPRESA, PARTE, { nombre: 'Ausente' }, admin)).rejects.toBeInstanceOf(NotFoundError);
    pg.queryOne.mockRejectedValueOnce({ code: '23505' });
    await expect(servicio.actualizar(EMPRESA, PARTE, { identificador_valor: 'repetido' }, admin)).rejects.toMatchObject({ codigo: 'CONFLICTO' });
  });

  it('lista partes como administrador y valida acceso de miembro y UUID', async () => {
    const pg = crearPg({ query: vi.fn().mockResolvedValue([parteFila]) });
    const servicio = new PartesService(pg as never);

    await expect(servicio.listar(EMPRESA, {}, admin)).resolves.toEqual([parteFila]);
    await expect(servicio.listar('empresa-invalida', {}, admin)).rejects.toMatchObject({ codigo: 'EMPRESA_INVALIDA' });

    pg.queryOne.mockResolvedValueOnce(null);
    await expect(servicio.listar(EMPRESA, {}, actor)).rejects.toBeInstanceOf(ForbiddenError);
    await expect(servicio.crear(EMPRESA, { tipo: 'persona', nombre: 'Ana' }, { ...actor, rol: 'cliente' } as never))
      .rejects.toBeInstanceOf(ForbiddenError);
  });

  it('lista, actualiza y elimina contactos con errores de validación', async () => {
    const pg = crearPg({
      query: vi.fn().mockResolvedValue([contactoFila]),
      queryOne: vi.fn()
        .mockResolvedValueOnce(parteFila)
        .mockResolvedValueOnce(parteFila).mockResolvedValueOnce(contactoFila)
        .mockResolvedValueOnce(parteFila).mockResolvedValueOnce(null),
    });
    const servicio = new PartesService(pg as never);

    await expect(servicio.listarContactos(EMPRESA, PARTE, admin)).resolves.toEqual([contactoFila]);
    await expect(servicio.actualizarContacto(EMPRESA, PARTE, contactoFila.id, {
      tipo: 'telefono', valor: '8888', etiqueta: 'móvil', es_principal: true, metadata: { fuente: 'test' },
    }, admin)).resolves.toEqual(contactoFila);
    await expect(servicio.eliminarContacto(EMPRESA, PARTE, contactoFila.id, admin)).rejects.toBeInstanceOf(NotFoundError);
    pg.queryOne.mockResolvedValueOnce(parteFila).mockResolvedValueOnce({ id: contactoFila.id });
    await expect(servicio.eliminarContacto(EMPRESA, PARTE, contactoFila.id, admin)).resolves.toEqual({ ok: true });
    pg.queryOne.mockResolvedValueOnce(parteFila);
    await expect(servicio.actualizarContacto(EMPRESA, PARTE, 'no-es-uuid', {}, admin)).rejects.toMatchObject({ codigo: 'CONTACTO_INVALIDO' });
  });

  it('traduce conflictos al crear o actualizar un contacto', async () => {
    const pg = crearPg({
      queryOne: vi.fn().mockResolvedValueOnce(parteFila).mockRejectedValueOnce({ code: '23505' }),
    });
    const servicio = new PartesService(pg as never);
    await expect(servicio.crearContacto(EMPRESA, PARTE, { tipo: 'email', valor: 'duplicado@example.test' }, admin))
      .rejects.toMatchObject({ codigo: 'CONFLICTO' });

    pg.queryOne.mockResolvedValueOnce(parteFila).mockRejectedValueOnce({ code: '23505' });
    await expect(servicio.actualizarContacto(EMPRESA, PARTE, contactoFila.id, { valor: 'duplicado@example.test' }, admin))
      .rejects.toMatchObject({ codigo: 'CONFLICTO' });
  });

  it('gestiona direcciones y valida tipos, límites y UUIDs', async () => {
    const pg = crearPg({
      query: vi.fn().mockResolvedValue([direccionFila]),
      queryOne: vi.fn()
        .mockResolvedValueOnce(parteFila)
        .mockResolvedValueOnce(parteFila).mockResolvedValueOnce(direccionFila)
        .mockResolvedValueOnce(parteFila).mockResolvedValueOnce(direccionFila)
        .mockResolvedValueOnce(parteFila).mockResolvedValueOnce(direccionFila),
    });
    const servicio = new PartesService(pg as never);

    await expect(servicio.listarDirecciones(EMPRESA, PARTE, admin)).resolves.toEqual([direccionFila]);
    await expect(servicio.crearDireccion(EMPRESA, PARTE, {
      tipo: 'entrega', linea_1: 'Calle 1', ciudad: 'Managua', latitud: 12.1, longitud: -86.2,
    }, admin)).resolves.toEqual(direccionFila);
    await expect(servicio.actualizarDireccion(EMPRESA, PARTE, direccionFila.id, {
      ciudad: 'León', referencias: 'Frente al parque', es_principal: true,
    }, admin)).resolves.toEqual(direccionFila);
    await expect(servicio.eliminarDireccion(EMPRESA, PARTE, direccionFila.id, admin)).resolves.toEqual({ ok: true });
    pg.queryOne.mockResolvedValueOnce(parteFila);
    await expect(servicio.crearDireccion(EMPRESA, PARTE, { tipo: 'invalido' as never, linea_1: 'x' }, admin))
      .rejects.toMatchObject({ codigo: 'TIPO_DIRECCION_INVALIDO' });
    pg.queryOne.mockResolvedValueOnce(parteFila);
    await expect(servicio.actualizarDireccion(EMPRESA, PARTE, 'no-es-uuid', {}, admin))
      .rejects.toMatchObject({ codigo: 'DIRECCION_INVALIDA' });
  });

  it('crea, lista y desactiva relaciones entre partes', async () => {
    const destino = { ...parteFila, id: '99999999-aaaa-4bbb-8ccc-dddddddddddd', nombre: 'Empresa' };
    const pg = crearPg({
      query: vi.fn().mockResolvedValue([{ ...relacionFila, relacionada_id: destino.id, relacionada_nombre: destino.nombre, relacionada_tipo: destino.tipo }]),
      queryOne: vi.fn()
        .mockResolvedValueOnce(parteFila)
        .mockResolvedValueOnce(parteFila).mockResolvedValueOnce(destino).mockResolvedValueOnce(relacionFila)
        .mockResolvedValueOnce(parteFila).mockResolvedValueOnce({ id: relacionFila.id }),
    });
    const servicio = new PartesService(pg as never);

    await expect(servicio.listarRelaciones(EMPRESA, PARTE, admin)).resolves.toEqual([
      expect.objectContaining({ parte_relacionada: expect.objectContaining({ id: destino.id, nombre: destino.nombre }) }),
    ]);
    await expect(servicio.crearRelacion(EMPRESA, PARTE, destino.id, ' Cliente ', { fuente: 'test' }, admin)).resolves.toEqual(relacionFila);
    await expect(servicio.eliminarRelacion(EMPRESA, PARTE, relacionFila.id, admin)).resolves.toEqual({ ok: true });
    pg.queryOne.mockResolvedValueOnce(parteFila);
    await expect(servicio.crearRelacion(EMPRESA, PARTE, PARTE, 'cliente', undefined, admin))
      .rejects.toMatchObject({ codigo: 'RELACION_INVALIDA' });
    pg.queryOne.mockResolvedValueOnce(parteFila);
    await expect(servicio.crearRelacion(EMPRESA, PARTE, destino.id, 'cliente-', undefined, admin))
      .rejects.toMatchObject({ codigo: 'TIPO_RELACION_INVALIDO' });
  });

  it('cubre validaciones de normalización y relaciones duplicadas', async () => {
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValue(parteFila) });
    const servicio = new PartesService(pg as never);

    await expect(servicio.crear(EMPRESA, { tipo: 'persona', nombre: '' }, admin)).rejects.toMatchObject({ codigo: 'NOMBRE_PARTE_INVALIDO' });
    await expect(servicio.actualizar(EMPRESA, PARTE, { tipo: 'otro' as never }, admin)).rejects.toMatchObject({ codigo: 'TIPO_PARTE_INVALIDO' });
    await expect(servicio.actualizar(EMPRESA, PARTE, { estado: 'otro' as never }, admin)).rejects.toMatchObject({ codigo: 'ESTADO_PARTE_INVALIDO' });
    await expect(servicio.actualizar(EMPRESA, PARTE, { nombre: 'x'.repeat(241) }, admin)).rejects.toMatchObject({ codigo: 'NOMBRE_PARTE_INVALIDO' });
    await expect(servicio.crearContacto(EMPRESA, PARTE, { tipo: 'otro', valor: 'x'.repeat(241) }, admin)).rejects.toMatchObject({ codigo: 'VALOR_INVALIDO' });
    await expect(servicio.crearDireccion(EMPRESA, PARTE, { tipo: 'entrega', linea_1: 'x', longitud: 181 }, admin)).rejects.toMatchObject({ codigo: 'COORDENADA_INVALIDA' });
    await expect(servicio.crearRelacion(EMPRESA, PARTE, 'no-es-uuid', 'cliente', undefined, admin)).rejects.toMatchObject({ codigo: 'PARTE_DESTINO_INVALIDA' });

    pg.queryOne.mockResolvedValueOnce(parteFila).mockResolvedValueOnce(parteFila).mockRejectedValueOnce({ code: '23505' });
    await expect(servicio.crearRelacion(EMPRESA, PARTE, PARTE.replace('2222', '9999'), 'cliente', undefined, admin))
      .rejects.toMatchObject({ codigo: 'CONFLICTO' });

    pg.queryOne.mockResolvedValueOnce(parteFila).mockResolvedValueOnce(parteFila).mockRejectedValueOnce(new Error('database unavailable'));
    await expect(servicio.crearRelacion(EMPRESA, PARTE, PARTE.replace('2222', '9999'), 'cliente', undefined, admin))
      .rejects.toThrow('database unavailable');

    pg.queryOne.mockResolvedValueOnce(parteFila).mockRejectedValueOnce(new Error('database unavailable'));
    await expect(servicio.actualizarContacto(EMPRESA, PARTE, contactoFila.id, { valor: 'fallo@example.test' }, admin))
      .rejects.toThrow('database unavailable');
  });
});
