import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { ServiciosService } from './servicios.service';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';

const fila = {
  id: '11111111-2222-3333-4444-555555555555',
  empresa_id: EMPRESA,
  codigo: 'SERV-001',
  nombre: 'Instalación',
  descripcion: 'Instalación en sitio',
  categoria: 'servicios',
  marca: 'Core',
  codigo_barras: '7501234567890',
  precio_cents: 150000,
  costo_cents: 50000,
  unidad: 'Hora (h)',
  duracion_minutos: 60,
  estado: 'activo',
  creado_en: '2026-10-30T10:00:00.000Z',
  actualizado_en: '2026-10-30T10:00:00.000Z',
};

function crear() {
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(null),
  };
  const productos = { validarEmpresaCatalogo: vi.fn().mockResolvedValue(undefined) };
  const servicio = new ServiciosService(pg as never, productos as never);
  return { servicio, pg, productos };
}

const datos = {
  codigo: 'serv-001',
  nombre: 'Instalación',
  precio: '1500.00',
  costo: '500.00',
  duracion_minutos: 60,
  codigo_barras: '7501234567890',
};

describe('ServiciosService (R-PS-02)', () => {
  it('lista servicios con filtros y normaliza importes', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValue({ n: 1 });
    (pg.query as Mock).mockResolvedValue([fila]);

    const pagina = await servicio.listar({
      empresa_id: EMPRESA,
      q: 'inst',
      estado: 'activo',
      categoria: 'servicios',
      pagina: 2,
      limite: 10,
    });

    expect(pagina.total).toBe(1);
    expect(pagina.pagina).toBe(2);
    expect(pagina.items[0]).toMatchObject({
      codigo: 'SERV-001',
      precio: '1500.00',
      costo: '500.00',
      estado: 'activo',
    });
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('FROM catalog.servicios'),
      expect.arrayContaining(['%inst%', 'activo', 'servicios', 10, 10]),
    );
  });

  it('exige empresa al listar', async () => {
    const { servicio } = crear();
    await expect(servicio.listar({})).rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
  });

  it('obtiene el detalle y devuelve null con id inválido o inexistente', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(fila);
    await expect(servicio.encontrarPorId(fila.id, EMPRESA)).resolves.toMatchObject({
      id: fila.id,
    });
    await expect(servicio.encontrarPorId('no-uuid', EMPRESA)).resolves.toBeNull();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.encontrarPorId(fila.id, EMPRESA)).resolves.toBeNull();
  });

  it('crea un servicio normalizando código y verificando empresa', async () => {
    const { servicio, pg, productos } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce(null).mockResolvedValueOnce(fila);

    const resultado = await servicio.crear(datos, EMPRESA);

    expect(productos.validarEmpresaCatalogo).toHaveBeenCalledWith(EMPRESA);
    const insert = (pg.queryOne as Mock).mock.calls[2];
    expect(insert[0]).toContain('INSERT INTO catalog.servicios');
    expect(insert[1]).toContain('SERV-001');
    expect(resultado.id).toBe(fila.id);
  });

  it('rechaza códigos y códigos de barras duplicados', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ id: 'existente' });
    await expect(servicio.crear(datos, EMPRESA)).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce(null).mockResolvedValueOnce({ id: 'existente' });
    await expect(servicio.crear(datos, EMPRESA)).rejects.toBeInstanceOf(ConflictError);
  });

  it('valida código, nombre, precio y duración', async () => {
    const { servicio } = crear();
    await expect(servicio.crear({ ...datos, codigo: 'A' }, EMPRESA)).rejects.toMatchObject({
      codigo: 'CODIGO_INVALIDO',
    });
    await expect(servicio.crear({ ...datos, nombre: ' ' }, EMPRESA)).rejects.toMatchObject({
      codigo: 'NOMBRE_INVALIDO',
    });
    await expect(servicio.crear({ ...datos, precio: '-1' }, EMPRESA)).rejects.toBeInstanceOf(
      DomainError,
    );
    await expect(servicio.crear({ ...datos, duracion_minutos: 0 }, EMPRESA)).rejects.toMatchObject({
      codigo: 'DURACION_INVALIDA',
    });
  });

  it('actualiza los campos enviados y conserva los no enviados', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(fila)
      .mockResolvedValueOnce({ ...fila, nombre: 'Soporte', estado: 'inactivo' });

    const resultado = await servicio.actualizar(
      fila.id,
      { nombre: 'Soporte', estado: 'inactivo', duracion_minutos: 90 },
      EMPRESA,
    );

    expect(resultado).toMatchObject({ nombre: 'Soporte', estado: 'inactivo' });
    const update = (pg.queryOne as Mock).mock.calls[1];
    expect(update[0]).toContain('UPDATE catalog.servicios');
    expect(update[1]).toContain(90);
  });

  it('actualiza sin cambiar el código de barras', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(fila)
      .mockResolvedValueOnce({ ...fila, nombre: 'Soporte' });

    await servicio.actualizar(fila.id, { nombre: 'Soporte' }, EMPRESA);

    expect(pg.queryOne).toHaveBeenCalledTimes(2);
  });

  it('rechaza actualizar un servicio inexistente', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(
      servicio.actualizar(fila.id, { nombre: 'Soporte' }, EMPRESA),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('da de baja lógicamente y falla si no existe', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ id: fila.id });
    await expect(servicio.eliminar(fila.id, EMPRESA)).resolves.toBeUndefined();
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('eliminado_en = NOW()'),
      [fila.id, EMPRESA],
    );

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.eliminar(fila.id, EMPRESA)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('valida el código de barras al actualizar si cambia', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(fila).mockResolvedValueOnce({ id: 'otro' });
    await expect(
      servicio.actualizar(fila.id, { codigo_barras: '999' }, EMPRESA),
    ).rejects.toBeInstanceOf(ConflictError);
  });
});
