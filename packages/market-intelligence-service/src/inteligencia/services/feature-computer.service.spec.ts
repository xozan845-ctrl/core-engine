import { Logger } from '@nestjs/common';
import { FeatureComputerService } from './feature-computer.service';

describe('FeatureComputerService (features estadisticas de inteligencia)', () => {
  const crear = () => {
    const pg = { query: vi.fn().mockResolvedValue([]) };
    const servicio = new FeatureComputerService(pg as never);
    return { servicio, pg };
  };

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('calcula las features de SKU con promedio movil y desviacion estandar', async () => {
    const { servicio, pg } = crear();

    await servicio.computeFeatures();

    const consultas = pg.query.mock.calls.map(([sql]) => String(sql));
    expect(consultas).toHaveLength(5);
    const promedio = consultas.find((sql) => sql.includes("'ventas_avg_7d'"));
    const desviacion = consultas.find((sql) => sql.includes("'ventas_std_7d'"));
    expect(promedio).toContain("INTERVAL '7 days'");
    expect(promedio).toContain('ON CONFLICT (entity_type, entity_id, feature_name)');
    expect(desviacion).toContain('STDDEV_POP(total_diario)');
  });

  it('calcula tasa de efectividad y riesgo de churn por vendedor', async () => {
    const { servicio, pg } = crear();

    await servicio.computeFeatures();

    const consultas = pg.query.mock.calls.map(([sql]) => String(sql));
    const efectividad = consultas.find((sql) => sql.includes("'tasa_efectividad'"));
    const churn = consultas.find((sql) => sql.includes("'churn_risk_score'"));
    expect(efectividad).toContain('FROM intelligence.rendimiento_vendedor');
    expect(churn).toContain('WHEN COALESCE(tasa_efectividad, 0) < 0.5');
    expect(churn).toContain('WHEN COALESCE(tasa_efectividad, 0) < 0.75');
  });

  it('calcula densidad de ventas por celda geografica en 30 dias', async () => {
    const { servicio, pg } = crear();

    await servicio.computeFeatures();

    const consulta = pg.query.mock.calls
      .map(([sql]) => String(sql))
      .find((sql) => sql.includes("'densidad_30d'"));
    expect(consulta).toContain("'densidad_30d'");
    expect(consulta).toContain("INTERVAL '30 days'");
    expect(consulta).toContain('lat IS NOT NULL AND lng IS NOT NULL');
    expect(consulta).toContain("ROUND(lat::NUMERIC, 2)::TEXT || ',' || ROUND(lng::NUMERIC, 2)::TEXT");
  });

  it('registra el tiempo cuando todas las consultas terminan correctamente', async () => {
    const { servicio } = crear();
    const logSpy = vi.spyOn(Logger.prototype, 'log').mockImplementation(() => undefined);

    await servicio.computeFeatures();

    expect(logSpy).toHaveBeenCalledWith(expect.stringMatching(/^Features computadas en \d+ms$/));
  });

  it('absorbe y registra el error de una consulta sin relanzarlo', async () => {
    const { servicio, pg } = crear();
    const error = new Error('database unavailable');
    pg.query.mockRejectedValueOnce(error);
    const errorSpy = vi.spyOn(Logger.prototype, 'error').mockImplementation(() => undefined);

    await expect(servicio.computeFeatures()).resolves.toBeUndefined();

    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining('Error computando features: database unavailable'),
      expect.any(String),
    );
  });
});
