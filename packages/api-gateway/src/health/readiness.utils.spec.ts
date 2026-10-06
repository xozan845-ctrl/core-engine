import {
  evaluarReadiness,
  codigoReadiness,
  SERVICIOS_PRODUCCION,
  type FetchComo,
} from './readiness.utils';

/**
 * R-CD-12: un deploy roto debe verse como `degraded`/503, no como 200 "verde".
 */
describe('readiness.utils (readiness agregada del gateway)', () => {
  const servicios = [
    { nombre: 'identity', puerto: 3001 },
    { nombre: 'catalog', puerto: 3002 },
  ];

  it('debe marcar ok cuando todos los servicios responden ok', async () => {
    const fetchFn: FetchComo = jest.fn().mockResolvedValue({ ok: true, status: 200 });
    const estado = await evaluarReadiness(servicios, fetchFn);
    expect(estado.status).toBe('ok');
    expect(estado.servicios).toEqual({ identity: 'ok', catalog: 'ok' });
    expect(codigoReadiness(estado)).toBe(200);
  });

  it('debe marcar degraded cuando un servicio responde error', async () => {
    const fetchFn: FetchComo = jest
      .fn()
      .mockResolvedValueOnce({ ok: true, status: 200 })
      .mockResolvedValueOnce({ ok: false, status: 500 });
    const estado = await evaluarReadiness(servicios, fetchFn);
    expect(estado.status).toBe('degraded');
    expect(estado.servicios.catalog).toBe('error:500');
    expect(codigoReadiness(estado)).toBe(503);
  });

  it('debe marcar caido cuando un servicio no responde', async () => {
    const fetchFn: FetchComo = jest
      .fn()
      .mockRejectedValueOnce(new Error('ECONNREFUSED'))
      .mockResolvedValueOnce({ ok: true, status: 200 });
    const estado = await evaluarReadiness(servicios, fetchFn);
    expect(estado.status).toBe('degraded');
    expect(estado.servicios.identity).toBe('caido');
  });

  it('debe consultar el /health interno por nombre-servicio y puerto', async () => {
    const fetchFn: FetchComo = jest.fn().mockResolvedValue({ ok: true, status: 200 });
    await evaluarReadiness(servicios, fetchFn);
    const urls = (fetchFn as jest.Mock).mock.calls.map((c) => c[0]);
    expect(urls).toContain('http://identity-service:3001/health');
    expect(urls).toContain('http://catalog-service:3002/health');
  });

  it('debe cubrir los 7 servicios de produccion (incluye field, excluye commissions/intelligence)', () => {
    const nombres = SERVICIOS_PRODUCCION.map((s) => s.nombre);
    expect(nombres).toContain('field');
    expect(nombres).not.toContain('commissions');
    expect(nombres).not.toContain('market-intelligence');
    expect(nombres).toHaveLength(7);
  });
});
