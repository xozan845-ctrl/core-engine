import { PassThrough, Readable } from 'stream';
import type { Request, Response } from 'express';
import { crearProxyPublico } from './public-proxy.middleware';

const ORIGEN = 'https://kb-coleccion-internacional-ni.web.app';
const CREDENCIAL = 'stage:secreto-e2e';
const BASIC = `Basic ${Buffer.from(CREDENCIAL).toString('base64')}`;

function crearReq(opciones: {
  method?: string;
  url?: string;
  headers?: Record<string, string>;
  body?: string;
  ip?: string;
}) {
  const req = Readable.from(opciones.body ? [Buffer.from(opciones.body)] : []) as unknown as Request;
  Object.assign(req, {
    method: opciones.method ?? 'GET',
    url: opciones.url ?? '/api/v1/catalog/productos',
    originalUrl: `/api-proxy${opciones.url ?? '/api/v1/catalog/productos'}`,
    headers: opciones.headers ?? {},
    ip: opciones.ip ?? '203.0.113.10',
    socket: { remoteAddress: opciones.ip ?? '203.0.113.10' },
  });
  return req;
}

function crearRes() {
  const res = new PassThrough() as unknown as Response & {
    headers: Record<string, string>;
    cuerpo?: unknown;
    statusCode: number;
  };
  Object.assign(res, {
    headers: {},
    statusCode: 200,
    headersSent: false,
    status(codigo: number) {
      this.statusCode = codigo;
      return this;
    },
    setHeader(clave: string, valor: string) {
      this.headers[clave] = valor;
    },
    json(cuerpo: unknown) {
      this.cuerpo = cuerpo;
      this.end();
      return this;
    },
  });
  return res;
}

function respuesta(cuerpo = '{"ok":true}', status = 200) {
  return {
    status,
    headers: new Headers({
      'content-type': 'application/json',
      'x-ratelimit-remaining': '299',
    }),
    body: Readable.toWeb(Readable.from([Buffer.from(cuerpo)])),
  } as unknown as globalThis.Response;
}

function handler(opciones: Partial<Parameters<typeof crearProxyPublico>[0]> = {}) {
  return crearProxyPublico({
    basicAuth: CREDENCIAL,
    origenes: [ORIGEN],
    puerto: 18080,
    ...opciones,
  });
}

async function ejecutar(peticion: Request, res: ReturnType<typeof crearRes>) {
  const chunks: Buffer[] = [];
  res.on('data', (chunk: Buffer) => chunks.push(Buffer.from(chunk)));
  const fin = new Promise<void>((resolve) => res.on('end', () => resolve()));
  await handler()(peticion, res);
  await fin;
  return Buffer.concat(chunks).toString();
}

describe('crearProxyPublico (ADR-29)', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('debe añadir Basic en login y reenviar al gateway interno', async () => {
    const fetchMock = vi.fn().mockResolvedValue(respuesta());
    vi.stubGlobal('fetch', fetchMock);
    const req = crearReq({
      method: 'POST',
      url: '/api/v1/auth/login',
      headers: { origin: ORIGEN, 'content-type': 'application/json' },
      body: '{"correo":"a@b.c","contrasena":"x"}',
    });
    const res = crearRes();

    const salida = await ejecutar(req, res);

    expect(salida).toBe('{"ok":true}');
    expect(res.statusCode).toBe(200);
    expect(res.headers['Access-Control-Allow-Origin']).toBe(ORIGEN);
    expect(res.headers['access-control-allow-origin']).toBeUndefined();
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe('http://127.0.0.1:18080/api/v1/auth/login');
    expect(init.method).toBe('POST');
    expect(init.headers.authorization).toBe(BASIC);
    expect(init.headers['x-forwarded-for']).toBe('203.0.113.10');
    expect(Buffer.from(init.body).toString()).toContain('correo');
  });

  it('debe reenviar el Bearer del navegador en rutas autenticadas', async () => {
    const fetchMock = vi.fn().mockResolvedValue(respuesta());
    vi.stubGlobal('fetch', fetchMock);
    const req = crearReq({
      url: '/api/v1/orders',
      headers: { authorization: 'Bearer jwt-del-usuario', origin: ORIGEN },
    });

    await ejecutar(req, crearRes());

    expect(fetchMock.mock.calls[0][1].headers.authorization).toBe('Bearer jwt-del-usuario');
  });

  it('debe usar Basic cuando una ruta protegida no trae Bearer', async () => {
    const fetchMock = vi.fn().mockResolvedValue(respuesta());
    vi.stubGlobal('fetch', fetchMock);

    await ejecutar(crearReq({ url: '/api/v1/orders', headers: { origin: ORIGEN } }), crearRes());

    expect(fetchMock.mock.calls[0][1].headers.authorization).toBe(BASIC);
  });

  it('debe responder el preflight OPTIONS sin llamar al API', async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const res = crearRes();

    await ejecutar(
      crearReq({ method: 'OPTIONS', url: '/api/v1/auth/login', headers: { origin: ORIGEN } }),
      res,
    );

    expect(res.statusCode).toBe(204);
    expect(res.headers['Access-Control-Allow-Methods']).toContain('POST');
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('no debe devolver ACAO para un origen no permitido', async () => {
    const fetchMock = vi.fn().mockResolvedValue(respuesta());
    vi.stubGlobal('fetch', fetchMock);
    const res = crearRes();

    await ejecutar(crearReq({ headers: { origin: 'https://evil.example' } }), res);

    expect(res.headers['Access-Control-Allow-Origin']).toBeUndefined();
    expect(fetchMock).toHaveBeenCalled();
  });

  it('debe rechazar rutas fuera de /api/v1 sin llamar al API', async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const res = crearRes();

    await ejecutar(crearReq({ url: '/otro' }), res);

    expect(res.statusCode).toBe(404);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('debe devolver 413 cuando el cuerpo excede el máximo', async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
    const res = crearRes();
    const proxy = crearProxyPublico({
      basicAuth: CREDENCIAL,
      origenes: [ORIGEN],
      puerto: 18080,
      maxBodyBytes: 2,
    });

    await proxy(
      crearReq({ method: 'POST', url: '/api/v1/orders', body: '12345', headers: { origin: ORIGEN } }),
      res,
    );

    expect(res.statusCode).toBe(413);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('debe devolver 504 cuando el API agota el timeout', async () => {
    const error = Object.assign(new Error('timeout'), { name: 'TimeoutError' });
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(error));
    const res = crearRes();

    await ejecutar(crearReq({ headers: { origin: ORIGEN } }), res);

    expect(res.statusCode).toBe(504);
    expect(res.cuerpo).toMatchObject({ codigo: 'PROXY_TIMEOUT' });
  });

  it('debe propagar el estado de error del API', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(respuesta('{"codigo":"NO_AUTORIZADO"}', 401)));
    const res = crearRes();

    await ejecutar(crearReq({ headers: { origin: ORIGEN } }), res);

    expect(res.statusCode).toBe(401);
    expect(res.cuerpo).toBeUndefined();
  });
});
