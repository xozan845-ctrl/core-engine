import { DomainError, NotFoundError } from '@core/shared';
import { FieldService } from './field.service';

const crearPg = () => ({
  query: vi.fn(),
  queryOne: vi.fn(),
});

describe('FieldService', () => {
  let pg: ReturnType<typeof crearPg>;
  let service: FieldService;

  beforeEach(() => {
    pg = crearPg();
    service = new FieldService(pg as never);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('crea rutas persistiendo personalIds, defaults y aislamiento por tenant', async () => {
    pg.queryOne.mockResolvedValue({ id: 'ruta-1' });

    await service.crearRuta('tenant-a', {
      nombre: 'Ruta norte',
      personalIds: ['personal-1', 'personal-2'],
    });

    const [sql, values] = pg.queryOne.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain('INSERT INTO field.rutas');
    expect(sql).toContain('personal_asignado_ids');
    expect(values[0]).toBe('tenant-a');
    expect(values).toContain('pendiente');
    expect(values).toContain(JSON.stringify(['personal-1', 'personal-2']));
    expect(values.at(-3)).toBe(true);
    expect(values.at(-2)).toBeInstanceOf(Date);
    expect(values.at(-1)).toBeInstanceOf(Date);
  });

  it('falla de forma explícita cuando un insert no devuelve registro', async () => {
    pg.queryOne.mockResolvedValue(null);

    await expect(service.crearCliente('tenant-a', { nombreCompleto: 'Ana' })).rejects.toMatchObject({
      codigo: 'PERSISTENCIA_FALLIDA',
    });
  });

  it('actualiza ubicación usando el contrato público lat/lng/precision/timestamp', async () => {
    pg.queryOne
      .mockResolvedValueOnce({ id: 'personal-1' })
      .mockResolvedValueOnce({ id: 'personal-1', ubicacion_lat: 12.1 });

    await service.actualizarUbicacion('tenant-a', 'personal-1', {
      lat: 12.1,
      lng: -86.2,
      precision: 4,
      timestamp: '2026-10-14T20:00:00.000Z',
    });

    const [sql, values] = pg.queryOne.mock.calls[1] as [string, unknown[]];
    expect(sql).toContain('ubicacion_lat = $1');
    expect(sql).toContain('ubicacion_lng = $2');
    expect(sql).toContain('ubicacion_precision = $3');
    expect(sql).toContain('ubicacion_ts = $4');
    expect(values.slice(0, 4)).toEqual([12.1, -86.2, 4, '2026-10-14T20:00:00.000Z']);
    expect(values.slice(-2)).toEqual(['tenant-a', 'personal-1']);
  });

  it('proyecta la ubicación persistida al contrato de respuesta', async () => {
    pg.queryOne.mockResolvedValue({
      ubicacion_lat: 12.1,
      ubicacion_lng: -86.2,
      ubicacion_precision: 4,
      ubicacion_ts: '2026-10-14T20:00:00.000Z',
    });

    await expect(service.ubicacionPersonal('tenant-a', 'personal-1')).resolves.toEqual({
      personalId: 'personal-1',
      latitud: 12.1,
      longitud: -86.2,
      precision: 4,
      timestamp: '2026-10-14T20:00:00.000Z',
    });
  });

  it('serializa arrays al actualizar y devuelve NO_ENCONTRADO si no existe la ruta', async () => {
    pg.queryOne.mockResolvedValueOnce({ id: 'ruta-1' }).mockResolvedValueOnce(null);

    await service.actualizarRuta('tenant-a', 'ruta-1', { personalIds: ['personal-1'] });
    const [sql, values] = pg.queryOne.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain('personal_asignado_ids = $1');
    expect(values[0]).toBe(JSON.stringify(['personal-1']));

    await expect(
      service.actualizarRuta('tenant-a', 'ausente', { estado: 'en_curso' }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('una actualización vacía relee el registro y conserva el filtro tenant', async () => {
    pg.queryOne.mockResolvedValue({ id: 'personal-1' });

    await expect(service.actualizarPersonal('tenant-a', 'personal-1', {})).resolves.toEqual({
      id: 'personal-1',
    });

    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('tenant_id = $1 AND id = $2 AND deleted_at IS NULL'),
      ['tenant-a', 'personal-1'],
    );
  });

  it('lista pedidos con filtros parametrizados sin interpolar valores', async () => {
    pg.query.mockResolvedValue([]);

    await service.listarPedidos('tenant-a', {
      estado: 'en_camino',
      rutaId: 'ruta-1',
      clienteId: 'cliente-1',
    });

    const [sql, values] = pg.query.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain('estado = $2');
    expect(sql).toContain('ruta_id = $3');
    expect(sql).toContain('cliente_id = $4');
    expect(sql).toContain('LIMIT 2000');
    expect(values).toEqual(['tenant-a', 'en_camino', 'ruta-1', 'cliente-1']);
  });

  it('construye rangos válidos para tracking y ordena por registrado_en', async () => {
    pg.query.mockResolvedValue([]);

    await service.listarTracking('tenant-a', {
      personalId: 'personal-1',
      desde: '2026-10-14T00:00:00Z',
      hasta: '2026-10-15T00:00:00Z',
    });

    const [sql, values] = pg.query.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain('personal_id = $2');
    expect(sql).toContain('registrado_en >= $3');
    expect(sql).toContain('registrado_en <= $4');
    expect(sql).not.toContain('>= =');
    expect(sql).toContain('ORDER BY registrado_en DESC');
    expect(values).toEqual([
      'tenant-a',
      'personal-1',
      '2026-10-14T00:00:00Z',
      '2026-10-15T00:00:00Z',
    ]);
  });

  it('anexa paradas ordenadas a cada ruta', async () => {
    pg.query
      .mockResolvedValueOnce([{ id: 'ruta-1' }, { id: 'ruta-2' }])
      .mockResolvedValueOnce([
        { id: 'p2', ruta_id: 'ruta-1', orden: 2 },
        { id: 'p1', ruta_id: 'ruta-1', orden: 1 },
      ]);

    const rutas = await service.listarRutas('tenant-a', 'personal-1');

    expect(pg.query.mock.calls[0][0]).toContain('personal_asignado_ids ?| array[$2]');
    expect(rutas[0].paradas).toEqual([
      { id: 'p1', ruta_id: 'ruta-1', orden: 1 },
      { id: 'p2', ruta_id: 'ruta-1', orden: 2 },
    ]);
    expect(rutas[1].paradas).toEqual([]);
  });

  it('agrega paradas usando la ruta del path y no una ruta enviada en el body', async () => {
    pg.queryOne
      .mockResolvedValueOnce({ id: 'ruta-real' })
      .mockResolvedValueOnce({ id: 'parada-1', ruta_id: 'ruta-real' });

    await service.agregarParada('tenant-a', 'ruta-real', {
      nombre: 'Cliente A',
      rutaId: 'ruta-ajena',
    });

    const [sql, values] = pg.queryOne.mock.calls[1] as [string, unknown[]];
    expect(sql).toContain('ruta_id');
    expect(values).toContain('ruta-real');
    expect(values).not.toContain('ruta-ajena');
  });

  it('rechaza actualizar una parada ausente o perteneciente a otra ruta', async () => {
    pg.queryOne.mockResolvedValueOnce(null).mockResolvedValueOnce({
      id: 'parada-1',
      ruta_id: 'ruta-otra',
    });

    await expect(
      service.actualizarParada('tenant-a', 'ruta-1', 'ausente', { nombre: 'A' }),
    ).rejects.toBeInstanceOf(NotFoundError);
    await expect(
      service.actualizarParada('tenant-a', 'ruta-1', 'parada-1', { nombre: 'A' }),
    ).rejects.toMatchObject({ codigo: 'PARADA_INVALIDA' });
  });

  it('asigna personal y vehículo a una ruta y reporta ruta ausente', async () => {
    pg.queryOne.mockResolvedValueOnce({ id: 'ruta-1' }).mockResolvedValueOnce(null);

    await service.asignarRuta('tenant-a', 'ruta-1', {
      personalIds: ['personal-1'],
      vehiculoId: 'vehiculo-1',
    });
    const [sql, values] = pg.queryOne.mock.calls[0] as [string, unknown[]];
    expect(sql).toContain('personal_asignado_ids = $1');
    expect(sql).toContain('vehiculo_asignado_id = $2');
    expect(values[0]).toBe(JSON.stringify(['personal-1']));
    expect(values[1]).toBe('vehiculo-1');

    await expect(service.asignarRuta('tenant-a', 'ausente', {})).rejects.toBeInstanceOf(
      NotFoundError,
    );
  });

  it('registra lotes de tracking como operaciones independientes', async () => {
    pg.queryOne
      .mockResolvedValueOnce({ id: 'track-1' })
      .mockResolvedValueOnce({ id: 'track-2' });

    const result = await service.registrarTracking('tenant-a', [
      { personalId: 'p1', latitud: 12, longitud: -86 },
      { personalId: 'p2', latitud: 13, longitud: -85 },
    ]);

    expect(result).toEqual([{ id: 'track-1' }, { id: 'track-2' }]);
    expect(pg.queryOne).toHaveBeenCalledTimes(2);
  });

  it('guarda métricas de cumplimiento como JSON y exige resultado persistido', async () => {
    pg.queryOne.mockResolvedValueOnce({ id: 'cumplimiento-1' }).mockResolvedValueOnce(null);

    await service.guardarCumplimiento('tenant-a', {
      rutaId: 'ruta-1',
      fecha: '2026-10-14',
      metricas: { entregas: 8 },
    });
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining('ON CONFLICT (tenant_id, ruta_id, fecha)'),
      ['tenant-a', 'ruta-1', '2026-10-14', JSON.stringify({ entregas: 8 })],
    );

    await expect(
      service.guardarCumplimiento('tenant-a', {
        rutaId: 'ruta-1',
        fecha: '2026-10-15',
        metricas: {},
      }),
    ).rejects.toBeInstanceOf(DomainError);
  });

  it('sincroniza operaciones soportadas y aísla errores por registro', async () => {
    vi.spyOn(console, 'log').mockImplementation(() => undefined);
    pg.queryOne
      .mockResolvedValueOnce({ id: 'cliente-1' })
      .mockRejectedValueOnce(new Error('db no disponible'));

    const result = await service.sincronizar('tenant-a', [
      { tipo: 'desconocido', payload: { id: 'x' } },
      { tipo: 'cliente', payload: { id: 'cliente-1', nombreCompleto: 'Ana' } },
      { tipo: 'pedido', payload: { id: 'pedido-1', cliente: 'Ana' } },
    ]);

    expect(result).toEqual([
      { tipo: 'desconocido', id: 'x', ok: false, error: 'tipo de entidad no soportado' },
      { tipo: 'cliente', id: 'cliente-1', ok: true },
      { tipo: 'pedido', id: 'pedido-1', ok: false, error: 'db no disponible' },
    ]);
    expect(pg.queryOne.mock.calls[0][0]).toContain('ON CONFLICT (id) DO UPDATE');
  });

  it('aplica borrado lógico y filtros de asistencia, incidencias y visitas', async () => {
    pg.query.mockResolvedValue([]);

    await service.eliminarVehiculo('tenant-a', 'vehiculo-1');
    await service.listarAsistencia('tenant-a', {
      personalId: 'personal-1',
      fecha: '2026-10-14',
    });
    await service.listarIncidencias('tenant-a', { estado: 'abierta', rutaId: 'ruta-1' });
    await service.listarVisitas('tenant-a', { personalId: 'personal-1', fecha: '2026-10-14' });

    expect(pg.query.mock.calls[0]).toEqual([
      expect.stringContaining('SET deleted_at = NOW()'),
      ['tenant-a', 'vehiculo-1'],
    ]);
    expect(pg.query.mock.calls[1][0]).toContain('DATE(registrado_en) = $3');
    expect(pg.query.mock.calls[2][0]).toContain('ruta_id = $3');
    expect(pg.query.mock.calls[3][0]).toContain('DATE(created_at) = $3');
  });
});
