import 'reflect-metadata';
import { ProxyService, ServicioDestino } from './proxy.service';
import { GatewayAuthMiddleware, PoliticaRuta } from './gateway-auth.middleware';
import { TENANT_HEADER, PERSONAL_HEADER, PARTICIPACION_HEADER, ROLES } from '@core/shared';
import type { Request, Response } from 'express';

const destinos: ServicioDestino[] = [
  {
    nombre: 'catalog',
    url: 'http://catalog-service:3002',
    patrones: [{ patron: /^\/api\/v1\/catalog\//, metodos: ['*'] }],
  },
];

const politicas: PoliticaRuta[] = [
  { patron: /^\/api\/v1\/catalog\//, metodos: ['*'], roles: null },
];

function respuestaFetch(status: number, body: string): globalThis.Response {
  return { status, headers: {}, text: async () => body } as unknown as globalThis.Response;
}

function reqCon(headers: Record<string, string>): Request {
  return {
    originalUrl: '/api/v1/catalog/productos',
    method: 'GET',
    headers: { ...headers },
    body: undefined,
  } as unknown as Request;
}

function resMock(): Response {
  return {
    headersSent: false,
    status: vi.fn().mockReturnThis(),
    setHeader: vi.fn(),
    send: vi.fn(),
    json: vi.fn(),
  } as unknown as Response;
}

describe('ProxyService — scrub de cabeceras de contexto (R-GW-3 / R-DS-5)', () => {
  const fetchOriginal = globalThis.fetch;
  afterEach(() => {
    globalThis.fetch = fetchOriginal;
    vi.restoreAllMocks();
  });

  it('debe eliminar x-tenant y x-user-personal inyectados por el cliente cuando el token no los trae', async () => {
    const auth = { aplicar: (_req: Request, _res: Response, next: () => void) => next() } as unknown as GatewayAuthMiddleware;
    const fetchMock = vi.fn().mockResolvedValue(respuestaFetch(200, '{}'));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const servicio = new ProxyService(auth, destinos, politicas);

    await servicio.reenviar(
      reqCon({ 'x-user-id': 'atacante', 'x-tenant': 'tenant-ajeno', 'x-user-personal': 'personal-ajeno' }),
      resMock(),
    );

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers['x-user-id']).toBeUndefined();
    expect(init.headers[TENANT_HEADER]).toBeUndefined();
    expect(init.headers[PERSONAL_HEADER]).toBeUndefined();
  });

  it('debe reenviar x-tenant y x-user-personal cuando el gateway los resuelve del token', async () => {
    const auth = {
      aplicar: (req: Request, _res: Response, next: () => void) => {
        req.headers[TENANT_HEADER] = 'tenant-del-token';
        req.headers[PERSONAL_HEADER] = 'personal-del-token';
        next();
      },
    } as unknown as GatewayAuthMiddleware;
    const fetchMock = vi.fn().mockResolvedValue(respuestaFetch(200, '{}'));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const servicio = new ProxyService(auth, destinos, politicas);

    await servicio.reenviar(reqCon({}), resMock());

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers[TENANT_HEADER]).toBe('tenant-del-token');
    expect(init.headers[PERSONAL_HEADER]).toBe('personal-del-token');
  });

  it('debe eliminar x-user-participacion inyectada por el cliente cuando el token no la trae', async () => {
    const auth = { aplicar: (_req: Request, _res: Response, next: () => void) => next() } as unknown as GatewayAuthMiddleware;
    const fetchMock = vi.fn().mockResolvedValue(respuestaFetch(200, '{}'));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const servicio = new ProxyService(auth, destinos, politicas);

    await servicio.reenviar(reqCon({ [PARTICIPACION_HEADER]: '100' }), resMock());

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers[PARTICIPACION_HEADER]).toBeUndefined();
  });

  it('debe reenviar x-user-participacion cuando el gateway la resuelve del token', async () => {
    const auth = {
      aplicar: (req: Request, _res: Response, next: () => void) => {
        req.headers[PARTICIPACION_HEADER] = '20';
        next();
      },
    } as unknown as GatewayAuthMiddleware;
    const fetchMock = vi.fn().mockResolvedValue(respuestaFetch(200, '{}'));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const servicio = new ProxyService(auth, destinos, politicas);

    await servicio.reenviar(reqCon({}), resMock());

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers[PARTICIPACION_HEADER]).toBe('20');
  });
});

describe('ProxyService — query string en enrutado y politicas (R-GW-1 / R-GW-3)', () => {
  const fetchOriginal = globalThis.fetch;
  afterEach(() => {
    globalThis.fetch = fetchOriginal;
    vi.restoreAllMocks();
  });

  function reqPara(url: string, method = 'GET'): Request {
    return { originalUrl: url, method, headers: {}, body: undefined } as unknown as Request;
  }

  it('debe resolver una ruta publica anclada aunque lleve filtros en la query', async () => {
    const politicasConCatchAll: PoliticaRuta[] = [
      { patron: /^\/api\/v1\/catalog\/productos$/, metodos: ['GET'], roles: null },
      { patron: /^\/api\/v1\//, metodos: ['*'], roles: [] },
    ];
    let capturada: PoliticaRuta | null = null;
    const auth = {
      aplicar: (_req: Request, _res: Response, next: () => void, politica: PoliticaRuta | null) => {
        capturada = politica;
        next();
      },
    } as unknown as GatewayAuthMiddleware;
    const fetchMock = vi.fn().mockResolvedValue(respuestaFetch(200, '{}'));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const servicio = new ProxyService(auth, destinos, politicasConCatchAll);

    await servicio.reenviar(reqPara('/api/v1/catalog/productos?limite=1&pagina=2'), resMock());

    expect(capturada).toBe(politicasConCatchAll[0]);
    expect(capturada?.roles).toBeNull();
    expect(fetchMock).toHaveBeenCalledWith(
      'http://catalog-service:3002/api/v1/catalog/productos?limite=1&pagina=2',
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('debe resolver la politica y el destino de orders con query string', async () => {
    const destinosOrders: ServicioDestino[] = [
      {
        nombre: 'orders',
        url: 'http://orders-service:3004',
        patrones: [{ patron: /^\/api\/v1\/orders(\/|$)/, metodos: ['*'] }],
      },
    ];
    const politicasOrders: PoliticaRuta[] = [
      {
        patron: /^\/api\/v1\/orders\/admin\/todas$/,
        metodos: ['GET'],
        roles: [ROLES.SUPER_ADMIN, ROLES.ADMIN],
      },
      { patron: /^\/api\/v1\//, metodos: ['*'], roles: [] },
    ];
    let capturada: PoliticaRuta | null = null;
    const auth = {
      aplicar: (_req: Request, _res: Response, next: () => void, politica: PoliticaRuta | null) => {
        capturada = politica;
        next();
      },
    } as unknown as GatewayAuthMiddleware;
    const fetchMock = vi.fn().mockResolvedValue(respuestaFetch(200, '{}'));
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    const servicio = new ProxyService(auth, destinosOrders, politicasOrders);

    await servicio.reenviar(reqPara('/api/v1/orders/admin/todas?estado=pagada'), resMock());

    expect(capturada).toBe(politicasOrders[0]);
    expect(capturada?.roles).toEqual([ROLES.SUPER_ADMIN, ROLES.ADMIN]);
    expect(fetchMock).toHaveBeenCalledWith(
      'http://orders-service:3004/api/v1/orders/admin/todas?estado=pagada',
      expect.objectContaining({ method: 'GET' }),
    );
  });
});
