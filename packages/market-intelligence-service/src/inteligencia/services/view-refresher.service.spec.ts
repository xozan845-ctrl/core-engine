import { Logger } from '@nestjs/common';
import { ViewRefresherService } from './view-refresher.service';

const GAUGE_REFRESH_SECONDS = 'core_engine_intelligence_view_refresh_duration_seconds';

describe('ViewRefresherService (vistas materializadas de inteligencia)', () => {
  const crear = () => {
    const pg = { query: vi.fn().mockResolvedValue(undefined) };
    const metrics = { registrarGauge: vi.fn() };
    const servicio = new ViewRefresherService(pg as never, metrics as never);
    return { servicio, pg, metrics };
  };

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('refresca las dos vistas concurrentes y registra la duracion en segundos', async () => {
    const { servicio, pg, metrics } = crear();
    await servicio.refreshMaterializedViews();

    expect(pg.query).toHaveBeenNthCalledWith(
      1,
      'REFRESH MATERIALIZED VIEW CONCURRENTLY intelligence.mv_resumen_ejecutivo',
    );
    expect(pg.query).toHaveBeenNthCalledWith(
      2,
      'REFRESH MATERIALIZED VIEW CONCURRENTLY intelligence.mv_tendencias',
    );
    expect(metrics.registrarGauge).toHaveBeenCalledWith(GAUGE_REFRESH_SECONDS, expect.any(Number));
    expect(metrics.registrarGauge.mock.calls[0][1]).toBeGreaterThanOrEqual(0);
  });

  it('registra el inicio y el fin del refresco cuando termina correctamente', async () => {
    const { servicio } = crear();
    const debugSpy = vi.spyOn(Logger.prototype, 'debug').mockImplementation(() => undefined);

    await servicio.refreshMaterializedViews();

    expect(debugSpy).toHaveBeenNthCalledWith(
      1,
      'Iniciando refresco concurrente de vistas materializadas...',
    );
    expect(debugSpy).toHaveBeenNthCalledWith(2, expect.stringMatching(/^Refresco concurrente completado en \d+ms$/));
  });

  it('absorbe un error de la primera vista y evita continuar el refresco', async () => {
    const { servicio, pg, metrics } = crear();
    const error = new Error('first view unavailable');
    pg.query.mockRejectedValueOnce(error);
    const errorSpy = vi.spyOn(Logger.prototype, 'error').mockImplementation(() => undefined);

    await expect(servicio.refreshMaterializedViews()).resolves.toBeUndefined();

    expect(pg.query).toHaveBeenCalledTimes(1);
    expect(metrics.registrarGauge).not.toHaveBeenCalled();
    expect(errorSpy).toHaveBeenCalledWith(
      'Error refrescando vistas materializadas: first view unavailable',
      expect.any(String),
    );
  });

  it('absorbe un error de la segunda vista sin registrar una duracion parcial', async () => {
    const { servicio, pg, metrics } = crear();
    const error = new Error('second view unavailable');
    pg.query.mockResolvedValueOnce(undefined).mockRejectedValueOnce(error);
    const errorSpy = vi.spyOn(Logger.prototype, 'error').mockImplementation(() => undefined);

    await expect(servicio.refreshMaterializedViews()).resolves.toBeUndefined();

    expect(pg.query).toHaveBeenCalledTimes(2);
    expect(metrics.registrarGauge).not.toHaveBeenCalled();
    expect(errorSpy).toHaveBeenCalledWith(
      'Error refrescando vistas materializadas: second view unavailable',
      expect.any(String),
    );
  });
});
