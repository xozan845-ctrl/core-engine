import { DataQualityService } from './data-quality.service';

describe('DataQualityService (contrato de calidad de ventas geolocalizadas)', () => {
  const crear = () => {
    const metrics = { incrementarContador: vi.fn() };
    const pg = { query: vi.fn().mockResolvedValue(undefined) };
    const servicio = new DataQualityService(metrics as never, pg as never);
    return { servicio, metrics, pg };
  };

  const ventaBase = (sobre: Record<string, unknown> = {}) => ({
    order_id: 'o-1',
    vendedor_id: 'v-1',
    skus: ['SKU-1'],
    monto_cents: 1000,
    ...sobre,
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('marca HIGH cuando todos los campos opcionales están completos', () => {
    const { servicio, metrics } = crear();
    const resultado = servicio.validarVentaGeolocalizada(
      ventaBase({
        lat: 12.1,
        lng: -86.2,
        precision: 5,
        velocidad: 20,
        rumbo: 180,
        tipo_actividad: 'visita',
        resultado_visita: 'exitosa',
        distancia_cliente_metros: 250,
        rango_edad: '25-34',
        genero: 'NS',
      }),
    );

    expect(resultado).toEqual({ isValid: true, errors: [], completenessScore: 1, qualityTier: 'HIGH' });
    expect(metrics.incrementarContador).toHaveBeenCalledWith(
      'core_engine_intelligence_data_quality_high_total',
    );
    expect(metrics.incrementarContador).toHaveBeenCalledTimes(1);
  });

  it('marca MEDIUM con más de la mitad de opcionales y LOW con ninguno', () => {
    const medio = crear();
    expect(
      medio.servicio.validarVentaGeolocalizada(
        ventaBase({ lat: 12, lng: -86, precision: 4, velocidad: 10, rumbo: 90, tipo_actividad: 'venta' }),
      ),
    ).toMatchObject({ isValid: true, completenessScore: 0.6, qualityTier: 'MEDIUM' });
    expect(medio.metrics.incrementarContador).toHaveBeenCalledWith(
      'core_engine_intelligence_data_quality_medium_total',
    );

    const bajo = crear();
    expect(bajo.servicio.validarVentaGeolocalizada(ventaBase())).toMatchObject({
      isValid: true,
      completenessScore: 0,
      qualityTier: 'LOW',
    });
    expect(bajo.metrics.incrementarContador).toHaveBeenCalledWith(
      'core_engine_intelligence_data_quality_low_total',
    );
  });

  it('cuenta como completo el valor 0 y descarta opcionales vacíos o nulos', () => {
    const { servicio } = crear();
    const resultado = servicio.validarVentaGeolocalizada(
      ventaBase({ lat: 0, lng: 0, precision: 0, velocidad: 0, rumbo: 0, tipo_actividad: '', resultado_visita: '' }),
    );

    expect(resultado.isValid).toBe(true);
    expect(resultado.completenessScore).toBe(0.5);
    expect(resultado.qualityTier).toBe('LOW');
  });

  it('rechaza campos requeridos ausentes y propiedades desconocidas', () => {
    const { servicio, metrics } = crear();
    const resultado = servicio.validarVentaGeolocalizada({ order_id: 'o-1', inesperado: true });

    expect(resultado.isValid).toBe(false);
    expect(resultado.errors.some((error) => error.includes('must have required property'))).toBe(true);
    expect(resultado.errors.some((error) => error.includes('must NOT have additional properties'))).toBe(true);
    expect(metrics.incrementarContador).toHaveBeenCalledWith(
      'core_engine_intelligence_invalid_events_total',
    );
    expect(metrics.incrementarContador).toHaveBeenCalledWith(
      'core_engine_intelligence_data_quality_low_total',
    );
    expect(metrics.incrementarContador).toHaveBeenCalledTimes(2);
  });

  it('aplica reglas de negocio de monto positivo y rango geográfico', () => {
    const { servicio } = crear();
    const resultado = servicio.validarVentaGeolocalizada(ventaBase({ monto_cents: 0, lat: 91 }));

    expect(resultado.isValid).toBe(false);
    expect(resultado.errors).toContain('monto_cents debe ser positivo');
    expect(resultado.errors).toContain('lat fuera de rango válido [-90, 90]');
  });

  it('rechaza tipos y valores fuera del schema aunque las reglas de negocio no se activen', () => {
    const { servicio } = crear();
    const resultado = servicio.validarVentaGeolocalizada(
      ventaBase({ skus: [], monto_cents: 100.5, lng: 181, genero: 'X' }),
    );

    expect(resultado.isValid).toBe(false);
    expect(resultado.errors.length).toBeGreaterThanOrEqual(4);
    expect(resultado.errors.some((error) => error.includes('/skus'))).toBe(true);
    expect(resultado.errors.some((error) => error.includes('/lng'))).toBe(true);
  });

  it('persiste un evento inválido serializando el payload y normalizando event_id ausente', async () => {
    const { servicio, pg } = crear();
    const payload = ventaBase({ monto_cents: 0 });

    await servicio.registrarEventoInvalido(undefined, 'venta.geolocalizada', ['monto inválido'], payload);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO intelligence.invalid_events'),
      [null, 'venta.geolocalizada', ['monto inválido'], JSON.stringify(payload)],
    );
  });

  it('no relanza errores de auditoría al persistir eventos inválidos', async () => {
    const { servicio, pg } = crear();
    pg.query.mockRejectedValueOnce(new Error('database unavailable'));

    await expect(servicio.registrarEventoInvalido('e-1', 'venta.geolocalizada', ['error'], {})).resolves.toBeUndefined();
  });
});
