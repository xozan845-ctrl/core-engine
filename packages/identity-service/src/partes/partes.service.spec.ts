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

describe('PartesService — cobertura de ramas y errores', () => {
  const OTRA_PARTE = '33333333-4444-4555-8666-777777777777';
  const CONTACTO = '66666666-7777-4888-8999-aaaaaaaaaaaa';
  const DIRECCION = '77777777-8888-4999-8aaa-bbbbbbbbbbbb';
  const RELACION = '88888888-9999-4aaa-8bbb-cccccccccccc';

  it('lista sin filtros con el límite por defecto', async () => {
    const pg = crearPg({ query: vi.fn().mockResolvedValue([]) });
    await new PartesService(pg as never).listar(EMPRESA, {}, actor);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('LIMIT $2'), [EMPRESA, 50]);
  });

  it('obtiene una parte existente y falla si no existe', async () => {
    const parte = { id: PARTE, empresa_id: EMPRESA };
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce(parte) });
    await expect(new PartesService(pg as never).obtener(EMPRESA, PARTE, actor)).resolves.toEqual(parte);

    const vacio = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce(null) });
    await expect(new PartesService(vacio as never).obtener(EMPRESA, PARTE, actor)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('rechaza identificadores UUID inválidos', async () => {
    const servicio = new PartesService(crearPg() as never);
    await expect(servicio.obtener(EMPRESA, 'no-uuid', actor)).rejects.toMatchObject({ codigo: 'PARTE_INVALIDA' });
  });

  it('actualiza una parte y traduce la violación de unicidad', async () => {
    const fila = { id: PARTE, nombre: 'Ana' };
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce(fila) });
    await expect(new PartesService(pg as never).actualizar(EMPRESA, PARTE, { nombre: ' Ana ' }, actor)).resolves.toEqual(fila);

    const dup = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockRejectedValueOnce({ code: '23505' }) });
    await expect(new PartesService(dup as never).actualizar(EMPRESA, PARTE, { nombre: 'Ana' }, actor)).rejects.toMatchObject({ codigo: 'CONFLICTO' });

    const vacio = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce(null) });
    await expect(new PartesService(vacio as never).actualizar(EMPRESA, PARTE, { nombre: 'Ana' }, actor)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('valida tipo, estado y longitud del nombre al normalizar', async () => {
    const servicio = new PartesService(crearPg() as never);
    await expect(servicio.actualizar(EMPRESA, PARTE, { tipo: 'robot' as never }, actor)).rejects.toMatchObject({ codigo: 'TIPO_PARTE_INVALIDO' });
    await expect(servicio.actualizar(EMPRESA, PARTE, { estado: 'raro' as never }, actor)).rejects.toMatchObject({ codigo: 'ESTADO_PARTE_INVALIDO' });
    await expect(servicio.actualizar(EMPRESA, PARTE, { nombre: 'x'.repeat(241) }, actor)).rejects.toMatchObject({ codigo: 'NOMBRE_PARTE_INVALIDO' });
  });

  it('lista contactos, direcciones y relaciones con la parte relacionada', async () => {
    const relacion = { id: RELACION, relacionada_id: OTRA_PARTE, relacionada_nombre: 'Luis', relacionada_tipo: 'persona' };
    const pg = crearPg({
      queryOne: vi.fn().mockResolvedValue({ 1: 1 }),
      query: vi.fn().mockResolvedValueOnce([{ id: CONTACTO }]).mockResolvedValueOnce([{ id: DIRECCION }]).mockResolvedValueOnce([relacion]),
    });
    const servicio = new PartesService(pg as never);

    await expect(servicio.listarContactos(EMPRESA, PARTE, actor)).resolves.toHaveLength(1);
    await expect(servicio.listarDirecciones(EMPRESA, PARTE, actor)).resolves.toHaveLength(1);
    const relaciones = await servicio.listarRelaciones(EMPRESA, PARTE, actor);
    expect(relaciones[0].parte_relacionada).toEqual({ id: OTRA_PARTE, nombre: 'Luis', tipo: 'persona' });
  });

  it('crea y actualiza contactos, con errores de validación y unicidad', async () => {
    const contacto = { id: CONTACTO };
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValue({ id: CONTACTO }) });
    const servicio = new PartesService(pg as never);
    await expect(servicio.crearContacto(EMPRESA, PARTE, { tipo: 'email', valor: 'a@b.c' }, actor)).resolves.toEqual({ id: CONTACTO });

    const sinValor = new PartesService(crearPg() as never);
    await expect(sinValor.crearContacto(EMPRESA, PARTE, { tipo: 'email', valor: '' }, actor)).rejects.toMatchObject({ codigo: 'CONTACTO_INVALIDO' });
    await expect(sinValor.crearContacto(EMPRESA, PARTE, { tipo: 'fax' as never, valor: 'x' }, actor)).rejects.toMatchObject({ codigo: 'TIPO_CONTACTO_INVALIDO' });

    const dup = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockRejectedValueOnce({ code: '23505' }) });
    await expect(new PartesService(dup as never).crearContacto(EMPRESA, PARTE, { tipo: 'email', valor: 'a@b.c' }, actor)).rejects.toMatchObject({ codigo: 'CONFLICTO' });

    const actualiza = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce({ id: CONTACTO }) });
    await expect(new PartesService(actualiza as never).actualizarContacto(EMPRESA, PARTE, CONTACTO, { valor: 'b@c.d' }, actor)).resolves.toEqual({ id: CONTACTO });
    await expect(new PartesService(crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce(null) }) as never).actualizarContacto(EMPRESA, PARTE, CONTACTO, {}, actor)).rejects.toBeInstanceOf(NotFoundError);
    expect(contacto.id).toBe(CONTACTO);
  });

  it('elimina contactos, direcciones y relaciones existentes o falla con 404', async () => {
    const exists = crearPg({ queryOne: vi.fn().mockResolvedValue({ 1: 1 }) });
    const servicio = new PartesService(exists as never);
    await expect(servicio.eliminarContacto(EMPRESA, PARTE, CONTACTO, actor)).resolves.toEqual({ ok: true });
    await expect(servicio.eliminarDireccion(EMPRESA, PARTE, DIRECCION, actor)).resolves.toEqual({ ok: true });
    await expect(servicio.eliminarRelacion(EMPRESA, PARTE, RELACION, actor)).resolves.toEqual({ ok: true });

    const missing = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce(null) });
    await expect(new PartesService(missing as never).eliminarContacto(EMPRESA, PARTE, CONTACTO, actor)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('crea y actualiza direcciones con defaults y errores', async () => {
    const direccion = { id: DIRECCION };
    const pg = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce(direccion) });
    const servicio = new PartesService(pg as never);
    await expect(servicio.crearDireccion(EMPRESA, PARTE, { tipo: 'fiscal', linea_1: 'Calle 1' }, actor)).resolves.toEqual(direccion);

    const sinLinea = new PartesService(crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ 1: 1 }) }) as never);
    await expect(sinLinea.crearDireccion(EMPRESA, PARTE, { tipo: 'fiscal', linea_1: ' ' }, actor)).rejects.toMatchObject({ codigo: 'DIRECCION_INVALIDA' });

    const coord = new PartesService(crearPg() as never);
    await expect(coord.crearDireccion(EMPRESA, PARTE, { tipo: 'fiscal', linea_1: 'C', longitud: 181 }, actor)).rejects.toMatchObject({ codigo: 'COORDENADA_INVALIDA' });

    const act = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce({ id: DIRECCION }) });
    await expect(new PartesService(act as never).actualizarDireccion(EMPRESA, PARTE, DIRECCION, { ciudad: 'Managua' }, actor)).resolves.toEqual({ id: DIRECCION });

    const vacio = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce(null) });
    await expect(new PartesService(vacio as never).actualizarDireccion(EMPRESA, PARTE, DIRECCION, {}, actor)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('valida relaciones: destino, código, metadata y duplicados', async () => {
    const servicio = new PartesService(crearPg() as never);
    await expect(servicio.crearRelacion(EMPRESA, PARTE, 'no-uuid', 'cliente', undefined, actor)).rejects.toMatchObject({ codigo: 'PARTE_DESTINO_INVALIDA' });
    await expect(servicio.crearRelacion(EMPRESA, PARTE, OTRA_PARTE, 'Cliente Inválido', undefined, actor)).rejects.toMatchObject({ codigo: 'TIPO_RELACION_INVALIDO' });
    await expect(servicio.crearRelacion(EMPRESA, PARTE, OTRA_PARTE, 'cliente', [] as never, actor)).rejects.toMatchObject({ codigo: 'METADATA_INVALIDA' });

    const destino = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce(null) });
    await expect(new PartesService(destino as never).crearRelacion(EMPRESA, PARTE, OTRA_PARTE, 'cliente', undefined, actor)).rejects.toBeInstanceOf(NotFoundError);

    const ok = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce({ id: OTRA_PARTE }).mockResolvedValueOnce({ id: RELACION }) });
    await expect(new PartesService(ok as never).crearRelacion(EMPRESA, PARTE, OTRA_PARTE, ' Cliente ', undefined, actor)).resolves.toEqual({ id: RELACION });

    const dup = crearPg({ queryOne: vi.fn().mockResolvedValueOnce({ 1: 1 }).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce({ id: OTRA_PARTE }).mockRejectedValueOnce({ code: '23505' }) });
    await expect(new PartesService(dup as never).crearRelacion(EMPRESA, PARTE, OTRA_PARTE, 'cliente', undefined, actor)).rejects.toMatchObject({ codigo: 'CONFLICTO' });
  });

  it('aplica el control de acceso global, por tenant y por rol de escritura', async () => {
    const admin = { user_id: USUARIO, rol: 'admin' as const };
    const pgAdmin = crearPg({ query: vi.fn().mockResolvedValue([]) });
    await expect(new PartesService(pgAdmin as never).listar(EMPRESA, {}, admin)).resolves.toEqual([]);
    expect(pgAdmin.queryOne).not.toHaveBeenCalled();

    const sinTenant = { user_id: USUARIO, rol: 'empresario' as const };
    await expect(new PartesService(crearPg() as never).listar(EMPRESA, {}, sinTenant)).rejects.toBeInstanceOf(ForbiddenError);

    const soloLectura = { user_id: USUARIO, rol: 'comprador' as const, tenant_id: EMPRESA };
    await expect(new PartesService(crearPg() as never).crear(EMPRESA, { tipo: 'persona', nombre: 'Ana' }, soloLectura)).rejects.toBeInstanceOf(ForbiddenError);

    const sinMiembro = crearPg({ queryOne: vi.fn().mockResolvedValue(null) });
    await expect(new PartesService(sinMiembro as never).listar(EMPRESA, {}, actor)).rejects.toBeInstanceOf(ForbiddenError);
  });

  it('rechaza textos que exceden el máximo', async () => {
    const servicio = new PartesService(crearPg() as never);
    await expect(servicio.actualizar(EMPRESA, PARTE, { nombre_comercial: 'x'.repeat(241) }, actor)).rejects.toMatchObject({ codigo: 'VALOR_INVALIDO' });
  });
});
