import type { Mock } from 'vitest';
import { ConflictError, DomainError, NotFoundError } from '@core/shared';
import { SeriesService } from './series.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const ITEM = '22222222-2222-4222-8222-222222222222';
const SERIE = '33333333-3333-4333-8333-333333333333';
const UBICACION = '44444444-4444-4444-8444-444444444444';

const item = {
  id: ITEM,
  tipo: 'producto',
  track_inventory: true,
  track_serial: true,
  codigo: 'SKU-01',
  nombre: 'Telefono',
  garantia_meses: 12,
};
const serie = {
  id: SERIE,
  empresa_id: EMPRESA,
  item_id: ITEM,
  ubicacion_id: UBICACION,
  serial: 'SN-0001',
  imei: '356789012345678',
  estado: 'disponible' as const,
  order_id: null,
  vendido_en: null,
  garantia_hasta: null,
  devuelto_en: null,
  metadata: {},
  creado_en: '2026-11-12T00:00:00.000Z',
  actualizado_en: '2026-11-12T00:00:00.000Z',
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  return { servicio: new SeriesService(pg as never), pg };
}

describe('SeriesService (Fase P1.3)', () => {
  it('lista las unidades serializadas del item', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    (pg.query as Mock).mockResolvedValue([serie]);
    await expect(servicio.listar(EMPRESA, ITEM)).resolves.toEqual([serie]);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('FROM catalog.item_series'),
      [EMPRESA, ITEM],
    );
  });

  it('registra una unidad con serial, IMEI y ubicacion validos', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(item)
      .mockResolvedValueOnce({ id: UBICACION })
      .mockResolvedValueOnce(serie);
    await expect(
      servicio.crear(EMPRESA, ITEM, {
        serial: ' SN-0001 ',
        imei: '356789012345678',
        ubicacion_id: UBICACION,
        metadata: { color: 'negro' },
      }),
    ).resolves.toEqual(serie);
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      3,
      expect.stringContaining('INSERT INTO catalog.item_series'),
      [EMPRESA, ITEM, UBICACION, 'SN-0001', '356789012345678', '{"color":"negro"}'],
    );
  });

  it('exige activar la serializacion antes de registrar unidades', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ ...item, track_serial: false });
    await expect(
      servicio.crear(EMPRESA, ITEM, { serial: 'SN-1' }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('valida serial, IMEI y ubicacion', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    await expect(
      servicio.crear(EMPRESA, ITEM, { serial: 'SN 1' }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      servicio.crear(EMPRESA, ITEM, { serial: 'SN-1', imei: 'ABC' }),
    ).rejects.toBeInstanceOf(DomainError);
    (pg.queryOne as Mock).mockResolvedValueOnce(item).mockResolvedValueOnce(null);
    await expect(
      servicio.crear(EMPRESA, ITEM, { serial: 'SN-1', ubicacion_id: UBICACION }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('traduce serial o IMEI duplicado a 409', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(item).mockRejectedValueOnce({ code: '23505' });
    await expect(
      servicio.crear(EMPRESA, ITEM, { serial: 'SN-1' }),
    ).rejects.toBeInstanceOf(ConflictError);
  });

  it('actualiza una unidad y permite volver a disponible o darla de baja', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(serie)
      .mockResolvedValueOnce({ ...serie, estado: 'baja' });
    await expect(
      servicio.actualizar(EMPRESA, SERIE, { estado: 'baja', metadata: { motivo: 'roto' } }),
    ).resolves.toMatchObject({ estado: 'baja' });
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining('UPDATE catalog.item_series'),
      ['SN-0001', '356789012345678', UBICACION, 'baja', '{"motivo":"roto"}', SERIE, EMPRESA],
    );

    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ ...serie, estado: 'devuelto' })
      .mockResolvedValueOnce({ ...serie, estado: 'disponible', devuelto_en: null });
    await expect(
      servicio.actualizar(EMPRESA, SERIE, { estado: 'disponible' }),
    ).resolves.toMatchObject({ estado: 'disponible' });
  });

  it('rechaza estados gestionados por ordenes y duplicados al actualizar', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(serie);
    await expect(
      servicio.actualizar(EMPRESA, SERIE, { estado: 'vendido' }),
    ).rejects.toBeInstanceOf(DomainError);

    (pg.queryOne as Mock).mockResolvedValueOnce(serie).mockRejectedValueOnce({ code: '23505' });
    await expect(
      servicio.actualizar(EMPRESA, SERIE, { serial: 'SN-2' }),
    ).rejects.toBeInstanceOf(ConflictError);

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.actualizar(EMPRESA, SERIE, {})).rejects.toBeInstanceOf(NotFoundError);
    await expect(servicio.actualizar(EMPRESA, 'no-uuid', {})).rejects.toBeInstanceOf(DomainError);
  });

  it('consulta una unidad por serial y calcula la garantia vigente', async () => {
    const { servicio, pg } = crear();
    const detalle = {
      ...serie,
      estado: 'vendido' as const,
      sku: 'SKU-01',
      nombre_item: 'Telefono',
      garantia_meses: 12,
      garantia_hasta: '2027-11-12',
      garantia_vigente: true,
    };
    (pg.queryOne as Mock).mockResolvedValueOnce(detalle);
    await expect(servicio.buscarPorSerial(EMPRESA, 'sn-0001')).resolves.toEqual(detalle);
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('garantia_vigente'),
      [EMPRESA, 'sn-0001'],
    );

    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.buscarPorSerial(EMPRESA, 'SN-9999')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('activa la serializacion solo en productos inventariables', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce(item)
      .mockResolvedValueOnce({ item_id: ITEM, track_serial: true });
    await expect(servicio.habilitarSerializacion(EMPRESA, ITEM, true)).resolves.toEqual({
      item_id: ITEM,
      track_serial: true,
    });

    (pg.queryOne as Mock).mockResolvedValueOnce({ ...item, tipo: 'servicio', track_inventory: false });
    await expect(
      servicio.habilitarSerializacion(EMPRESA, ITEM, true),
    ).rejects.toBeInstanceOf(DomainError);

    (pg.queryOne as Mock).mockResolvedValueOnce(item);
    await expect(servicio.habilitarSerializacion(EMPRESA, 'no-uuid', true)).rejects.toBeInstanceOf(
      DomainError,
    );
  });
});
