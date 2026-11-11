import 'reflect-metadata';
import { firstValueFrom, of } from 'rxjs';
import { describe, expect, it } from 'vitest';
import {
  crearRespuestaApi,
  EnvelopeInterceptor,
  esRespuestaApi,
  rutaLlevaEnvelope,
} from './response-envelope.interceptor';

function contexto(ruta: string, statusCode = 200, tipo = 'http') {
  const req = { originalUrl: ruta, url: ruta, method: 'GET' };
  const res = { statusCode };
  return {
    getType: () => tipo,
    switchToHttp: () => ({
      getRequest: () => req,
      getResponse: () => res,
    }),
  } as never;
}

describe('EnvelopeInterceptor (R-C-1)', () => {
  it('envuelve una respuesta pública con data y meta', async () => {
    const salida = await firstValueFrom(
      new EnvelopeInterceptor().intercept(contexto('/api/v2/admin/reportes'), {
        handle: () => of({ gmv_mes_cents: 100 }),
      }),
    );

    expect(salida).toEqual({
      data: { gmv_mes_cents: 100 },
      meta: {
        timestamp: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T.*Z$/),
        path: '/api/v2/admin/reportes',
        method: 'GET',
        statusCode: 200,
      },
    });
  });

  it.each([
    '/internal/orders/o1',
    '/health',
    '/metrics',
    '/docs',
    '/docs-json',
    '/api/v2/storage/public/objetos/x',
  ])('no envuelve %s', async (ruta) => {
    const salida = await firstValueFrom(
      new EnvelopeInterceptor().intercept(contexto(ruta), {
        handle: () => of({ ok: true }),
      }),
    );
    expect(salida).toEqual({ ok: true });
  });

  it('no envuelve respuestas 204 ni undefined', async () => {
    const sinCuerpo = await firstValueFrom(
      new EnvelopeInterceptor().intercept(contexto('/api/v2/x', 204), {
        handle: () => of(undefined),
      }),
    );
    const indefinido = await firstValueFrom(
      new EnvelopeInterceptor().intercept(contexto('/api/v2/x'), {
        handle: () => of(undefined),
      }),
    );
    expect(sinCuerpo).toBeUndefined();
    expect(indefinido).toBeUndefined();
  });

  it('no envuelve contextos que no son HTTP', async () => {
    const salida = await firstValueFrom(
      new EnvelopeInterceptor().intercept(contexto('/api/v2/x', 200, 'rpc'), {
        handle: () => of({ ok: true }),
      }),
    );
    expect(salida).toEqual({ ok: true });
  });

  it('no duplica una respuesta que ya tiene forma de envelope', async () => {
    const yaEnvuelta = crearRespuestaApi({ ok: true }, {
      timestamp: '2026-11-11T00:00:00.000Z',
      path: '/api/v2/x',
      method: 'GET',
      statusCode: 200,
    });
    const salida = await firstValueFrom(
      new EnvelopeInterceptor().intercept(contexto('/api/v2/x'), {
        handle: () => of(yaEnvuelta),
      }),
    );
    expect(salida).toBe(yaEnvuelta);
  });

  it('reconoce rutas públicas y excluye las de infraestructura', () => {
    expect(rutaLlevaEnvelope('/api/v2/admin/reportes')).toBe(true);
    expect(rutaLlevaEnvelope('/internal/orders/o1')).toBe(false);
    expect(rutaLlevaEnvelope('/api/v2/storage/public/objetos/x')).toBe(false);
  });

  it('detecta la forma de envelope solo con data y meta completo', () => {
    expect(esRespuestaApi({ data: 1, meta: {} })).toBe(false);
    expect(esRespuestaApi({ data: 1 })).toBe(false);
    expect(esRespuestaApi({ data: 1, meta: { timestamp: '', path: '', method: '', statusCode: 200 } }))
      .toBe(true);
  });
});
