import { RateLimitMiddleware } from './rate-limit.middleware';

const crearReq = (ip = '203.0.113.10', path = '/api/v1/orders', method = 'GET') =>
  ({
    ip,
    socket: { remoteAddress: ip },
    path,
    method,
  }) as never;

const crearRes = () => {
  const res = {
    statusCode: 200,
    headers: {} as Record<string, string>,
    cuerpo: undefined as unknown,
    setHeader(clave: string, valor: string) {
      this.headers[clave] = valor;
    },
    status(codigo: number) {
      this.statusCode = codigo;
      return this;
    },
    json(body: unknown) {
      this.cuerpo = body;
      return this;
    },
  };
  return res;
};

describe('RateLimitMiddleware (ventana fija por IP)', () => {
  it('debe permitir hasta el limite y responder 429 despues', () => {
    const limite = new RateLimitMiddleware();
    const opciones = { limite: 2, ttlMs: 60_000, clave: 'test' };

    const res1 = crearRes();
    expect(limite.consumir(crearReq(), res1 as never, opciones)).toBe(true);
    expect(res1.headers['x-ratelimit-limit']).toBe('2');
    expect(res1.headers['x-ratelimit-remaining']).toBe('1');

    const res2 = crearRes();
    expect(limite.consumir(crearReq(), res2 as never, opciones)).toBe(true);
    expect(res2.headers['x-ratelimit-remaining']).toBe('0');

    const res3 = crearRes();
    expect(limite.consumir(crearReq(), res3 as never, opciones)).toBe(false);
    expect(res3.statusCode).toBe(429);
    expect(res3.headers['x-ratelimit-remaining']).toBe('0');
    expect(res3.headers['Retry-After']).toBeDefined();
    expect(res3.cuerpo).toMatchObject({ codigo: 'DEMASIADAS_PETICIONES' });
  });

  it('debe separar los cubos por IP y por nombre de cubo', () => {
    const limite = new RateLimitMiddleware();
    const opciones = { limite: 1, ttlMs: 60_000, clave: 'login' };

    expect(limite.consumir(crearReq('203.0.113.1'), crearRes() as never, opciones)).toBe(true);
    expect(limite.consumir(crearReq('203.0.113.2'), crearRes() as never, opciones)).toBe(true);
    expect(limite.consumir(crearReq('203.0.113.1'), crearRes() as never, { ...opciones, clave: 'ready' })).toBe(true);
    expect(limite.consumir(crearReq('203.0.113.1'), crearRes() as never, opciones)).toBe(false);
  });

  it('no debe consumir el cubo con imágenes públicas (ADR-28)', () => {
    const limite = new RateLimitMiddleware();
    const res = crearRes();
    for (let i = 0; i < 400; i += 1) {
      limite.use(
        crearReq('203.0.113.50', '/api/v1/storage/public/objetos/22222222-2222-4222-8222-222222222222'),
        res as never,
        vi.fn(),
      );
    }
    expect(res.headers['x-ratelimit-limit']).toBeUndefined();
    expect(res.statusCode).toBe(200);
  });

  it('debe aplicar el limite estricto de login solo al POST /api/v1/auth/login', () => {
    const limite = new RateLimitMiddleware();
    const res = crearRes();

    for (let i = 0; i < 10; i += 1) {
      expect(
        limite.use(crearReq('203.0.113.9', '/api/v1/auth/login', 'POST'), res as never, vi.fn()),
      ).toBeUndefined();
    }
    expect(res.statusCode).toBe(200);

    const resExcedido = crearRes();
    limite.use(crearReq('203.0.113.9', '/api/v1/auth/login', 'POST'), resExcedido as never, vi.fn());
    expect(resExcedido.statusCode).toBe(429);
  });
});
