import 'reflect-metadata';
import { PrediccionController } from './prediccion.controller';
import type { InteligenciaRepository } from '../repositories/inteligencia.repository';

function fabricar(): { repo: Record<string, jest.Mock>; controller: PrediccionController } {
  const repo = {
    obtenerForecastDemanda: jest.fn(),
    obtenerAnomalias: jest.fn(),
    obtenerScoreVendedor: jest.fn(),
    obtenerCalidadDatos: jest.fn(),
  };
  const controller = new PrediccionController(repo as unknown as InteligenciaRepository);
  return { repo, controller };
}

describe('PrediccionController (api/v1/inteligencia, R-U-10/11)', () => {
  it('debe calcular el forecast con dias por defecto 7', async () => {
    const { repo, controller } = fabricar();
    repo.obtenerForecastDemanda.mockResolvedValue({ sku: 'SKU-01', proyeccion: [] });
    await controller.forecastDemanda('SKU-01', undefined);
    expect(repo.obtenerForecastDemanda).toHaveBeenCalledWith('SKU-01', 7);
  });

  it('debe acotar los dias del forecast al rango 1..90', async () => {
    const { repo, controller } = fabricar();
    repo.obtenerForecastDemanda.mockResolvedValue({});
    await controller.forecastDemanda('SKU-01', '500');
    expect(repo.obtenerForecastDemanda).toHaveBeenCalledWith('SKU-01', 90);
    await controller.forecastDemanda('SKU-01', '0');
    expect(repo.obtenerForecastDemanda).toHaveBeenCalledWith('SKU-01', 7);
  });

  it('debe devolver anomalias con total y limite por defecto 50', async () => {
    const { repo, controller } = fabricar();
    repo.obtenerAnomalias.mockResolvedValue([{ venta_id: 'v-1' }]);
    const resultado = await controller.anomalias('SKU-01', 'u-v', undefined);
    expect(repo.obtenerAnomalias).toHaveBeenCalledWith({ sku: 'SKU-01', vendedor_id: 'u-v', limite: 50 });
    expect(resultado).toEqual({ total: 1, anomalias: [{ venta_id: 'v-1' }] });
  });

  it('debe acotar el limite de anomalias a 200', async () => {
    const { repo, controller } = fabricar();
    repo.obtenerAnomalias.mockResolvedValue([]);
    await controller.anomalias(undefined, undefined, '9999');
    expect(repo.obtenerAnomalias).toHaveBeenCalledWith({ sku: undefined, vendedor_id: undefined, limite: 200 });
  });

  it('debe devolver el score del vendedor o el mensaje de sin datos', async () => {
    const { repo, controller } = fabricar();
    repo.obtenerScoreVendedor.mockResolvedValue({ vendedor_id: 'u-v', churn: 0.2 });
    await expect(controller.scoreVendedor('u-v')).resolves.toEqual({ vendedor_id: 'u-v', churn: 0.2 });
    repo.obtenerScoreVendedor.mockResolvedValue(null);
    const sinDatos = await controller.scoreVendedor('u-x');
    expect(sinDatos).toMatchObject({ vendedor_id: 'u-x' });
    expect(sinDatos.mensaje).toContain('Sin datos');
  });

  it('debe exigir x-user-id en mi-score y resolver el score cuando llega', async () => {
    const { repo, controller } = fabricar();
    const sinHeader = await controller.miScore('');
    expect(sinHeader.mensaje).toContain('x-user-id');
    repo.obtenerScoreVendedor.mockResolvedValue({ vendedor_id: 'u-v' });
    await expect(controller.miScore('u-v')).resolves.toEqual({ vendedor_id: 'u-v' });
  });

  it('debe devolver la calidad de datos con horas por defecto 24 y tope 720', async () => {
    const { repo, controller } = fabricar();
    repo.obtenerCalidadDatos.mockResolvedValue({ tasa_rechazo: 0 });
    await controller.calidadDatos(undefined);
    expect(repo.obtenerCalidadDatos).toHaveBeenCalledWith(24);
    await controller.calidadDatos('9999');
    expect(repo.obtenerCalidadDatos).toHaveBeenCalledWith(720);
  });
});