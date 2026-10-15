import { beforeEach, describe, expect, it } from 'vitest';
import {
  METRICAS_HTTP,
  MetricsService,
  parametrizarRuta,
  validarNombreMetrica,
} from './metrics.service';

function series(texto: string, nombre: string): string[] {
  return texto.split('\n').filter((linea) => linea.startsWith(`${nombre}{`));
}

describe('MetricsService (R-TM-2/3/4)', () => {
  let metrics: MetricsService;

  beforeEach(() => {
    metrics = new MetricsService('orders-service');
    metrics.onModuleInit();
  });

  it('expone rate y duración con namespace, unidad y labels en español', async () => {
    metrics.registrarPeticion(
      'get',
      '/api/v1/orders/550e8400-e29b-41d4-a716-446655440000?include=items',
      200,
      1250,
    );

    const texto = await metrics.texto();
    const peticion = series(texto, METRICAS_HTTP.peticiones)[0];
    const duracion = series(texto, `${METRICAS_HTTP.latencia}_sum`)[0];
    for (const label of [
      'estado="200"',
      'metodo="GET"',
      'ruta="/api/v1/orders/:id"',
      'servicio="orders-service"',
    ]) {
      expect(peticion).toContain(label);
      expect(duracion).toContain(label);
    }
    expect(peticion).toMatch(/\} 1$/);
    expect(duracion).toMatch(/\} 1\.25$/);
    expect(texto).not.toContain('status=');
  });

  it('separa 5xx de los 4xx relevantes sin clasificar otros 4xx como señal', async () => {
    metrics.registrarPeticion('POST', '/api/v1/auth/login', 401, 10);
    metrics.registrarPeticion('POST', '/api/v1/orders', 409, 20);
    metrics.registrarPeticion('GET', '/api/v1/orders/no-existe', 404, 30);
    metrics.registrarPeticion('GET', '/api/v1/orders/123', 503, 40);

    const texto = await metrics.texto();
    const erroresCliente = series(texto, METRICAS_HTTP.erroresCliente);
    const erroresServidor = series(texto, METRICAS_HTTP.erroresServidor);
    expect(erroresCliente).toHaveLength(2);
    expect(erroresCliente.some((linea) => linea.includes('estado="401"'))).toBe(true);
    expect(erroresCliente.some((linea) => linea.includes('estado="409"'))).toBe(true);
    expect(erroresCliente.some((linea) => linea.includes('estado="404"'))).toBe(false);
    expect(erroresServidor).toHaveLength(1);
    expect(erroresServidor[0]).toContain('estado="503"');
  });

  it.each([
    ['/api/v1/orders/123?expand=true', '/api/v1/orders/:id'],
    ['/api/v1/orders/abc-123', '/api/v1/orders/:id'],
    ['/api/v1/orders/:orderId', '/api/v1/orders/:orderId'],
    ['//health?full=1', '/health'],
  ])('parametriza %s como %s', (raw, expected) => {
    expect(parametrizarRuta(raw)).toBe(expected);
  });

  it('rechaza nombres dinámicos fuera del contrato y valores no finitos', () => {
    expect(() => validarNombreMetrica('requests', 'counter')).toThrow('Métrica inválida');
    expect(() => validarNombreMetrica('core-engine_orders_created_total', 'counter')).toThrow(
      'Métrica inválida',
    );
    expect(() => validarNombreMetrica('core_engine_orders_created_items', 'counter')).toThrow(
      '_total',
    );
    expect(() => validarNombreMetrica('core_engine_orders_pending_total', 'gauge')).toThrow(
      'reservado',
    );
    expect(() => metrics.incrementarContador('core_engine_orders_created_total', -1)).toThrow(
      'no negativo',
    );
    expect(() => metrics.registrarGauge('core_engine_orders_pending_items', Number.NaN)).toThrow(
      'finito',
    );
  });

  it('registra métricas de negocio válidas y reutiliza su instancia', async () => {
    metrics.incrementarContador('core_engine_orders_created_total');
    metrics.incrementarContador('core_engine_orders_created_total', 2);
    metrics.registrarGauge('core_engine_orders_pending_items', 7);
    metrics.registrarGauge('core_engine_orders_pending_items', 4);

    const texto = await metrics.texto();
    expect(texto).toContain('core_engine_orders_created_total 3');
    expect(texto).toContain('core_engine_orders_pending_items 4');
  });
});
