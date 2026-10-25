import { edgeAuth, leerEdge } from './edge-auth.middleware';
import { firmarToken, ROLES } from '@core/shared';

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

const crearReq = (path: string, extra: Record<string, unknown> = {}) => ({
  path,
  method: 'GET',
  headers: {} as Record<string, unknown>,
  ...extra,
});

const basic = (credencial: string): string => `Basic ${Buffer.from(credencial).toString('base64')}`;

const SECRETO = 'test_secret_edge';

const accessToken = (secreto = SECRETO): string =>
  firmarToken(
    { sub: 'u-1', email: 'u@test', rol: ROLES.COMPRADOR, tipo: 'access' },
    secreto,
    '900s',
  );

describe('edgeAuth (endurecimiento del borde: gate de entornos y diagnostico)', () => {
  describe('sin gate de Basic Auth', () => {
    it('debe dejar pasar todo cuando los diagnosticos son publicos (dev)', () => {
      const next = vi.fn();
      edgeAuth({ diagnosticosPublicos: true, readyPublico: true })(
        crearReq('/metrics') as never,
        crearRes() as never,
        next,
      );
      edgeAuth({ diagnosticosPublicos: true, readyPublico: true })(
        crearReq('/docs') as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(2);
    });

    it('debe ocultar /metrics, /docs, /docs-json y /docs-yaml cuando no son publicos', () => {
      for (const ruta of ['/metrics', '/docs', '/docs/', '/docs-json', '/docs-yaml']) {
        const next = vi.fn();
        const res = crearRes();
        edgeAuth({ diagnosticosPublicos: false })(crearReq(ruta) as never, res as never, next);
        expect(next).not.toHaveBeenCalled();
        expect(res.statusCode).toBe(404);
        expect(res.cuerpo).toMatchObject({ codigo: 'NO_ENCONTRADO' });
      }
    });

    it('debe permitir el diagnostico con la clave interna', () => {
      const next = vi.fn();
      edgeAuth({ diagnosticosPublicos: false, claveInterna: 'clave-interna' })(
        crearReq('/metrics', { headers: { 'x-internal-key': 'clave-interna' } }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe ocultar el diagnostico con una clave interna incorrecta', () => {
      const next = vi.fn();
      const res = crearRes();
      edgeAuth({ diagnosticosPublicos: false, claveInterna: 'clave-interna' })(
        crearReq('/docs', { headers: { 'x-internal-key': 'otra' } }) as never,
        res as never,
        next,
      );
      expect(next).not.toHaveBeenCalled();
      expect(res.statusCode).toBe(404);
    });
  });

  describe('/ready (operativo: clave interna o READY_PUBLIC)', () => {
    it('debe ocultar /ready sin clave interna cuando READY_PUBLIC no esta activo', () => {
      const next = vi.fn();
      const res = crearRes();
      edgeAuth({ diagnosticosPublicos: false, claveInterna: 'interna' })(
        crearReq('/ready') as never,
        res as never,
        next,
      );
      expect(next).not.toHaveBeenCalled();
      expect(res.statusCode).toBe(404);
    });

    it('debe permitir /ready con la clave interna', () => {
      const next = vi.fn();
      edgeAuth({ diagnosticosPublicos: false, claveInterna: 'interna' })(
        crearReq('/ready', { headers: { 'x-internal-key': 'interna' } }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe permitir /ready sin clave si READY_PUBLIC=true (dev)', () => {
      const next = vi.fn();
      edgeAuth({ diagnosticosPublicos: false, readyPublico: true })(
        crearReq('/ready') as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('la clave interna debe abrir /ready sin exigir Basic (smoke de despliegue)', () => {
      const next = vi.fn();
      edgeAuth({ basicAuth: 'stage:secreto-e2e', diagnosticosPublicos: false, claveInterna: 'interna' })(
        crearReq('/ready', { headers: { 'x-internal-key': 'interna' } }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });
  });

  describe('con gate de Basic Auth (staging)', () => {
    const opciones = {
      basicAuth: 'stage:secreto-e2e',
      diagnosticosPublicos: false,
      jwtSecret: SECRETO,
      bypassOrigins: ['https://kb-coleccion-internacional-ni.web.app'],
    };

    it('debe exigir credencial y anunciarla con WWW-Authenticate', () => {
      const next = vi.fn();
      const res = crearRes();
      edgeAuth(opciones)(crearReq('/api/v1/orders') as never, res as never, next);
      expect(next).not.toHaveBeenCalled();
      expect(res.statusCode).toBe(401);
      expect(res.headers['WWW-Authenticate']).toContain('Basic');
      expect(res.cuerpo).toMatchObject({ codigo: 'NO_AUTORIZADO' });
    });

    it('debe rechazar una credencial incorrecta', () => {
      const next = vi.fn();
      const res = crearRes();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', { headers: { authorization: basic('stage:otra') } }) as never,
        res as never,
        next,
      );
      expect(next).not.toHaveBeenCalled();
      expect(res.statusCode).toBe(401);
    });

    it('debe permitir el paso con la credencial correcta', () => {
      const next = vi.fn();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', {
          headers: { authorization: basic('stage:secreto-e2e') },
        }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe rechazar un Bearer invalido (no puede saltarse el Basic)', () => {
      const next = vi.fn();
      const res = crearRes();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', { headers: { authorization: 'Bearer token-invalido' } }) as never,
        res as never,
        next,
      );
      expect(next).not.toHaveBeenCalled();
      expect(res.statusCode).toBe(401);
      expect(res.cuerpo).toMatchObject({ codigo: 'NO_AUTORIZADO' });
    });

    it('debe aceptar un Bearer de acceso valido sin exigir Basic', () => {
      const next = vi.fn();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', {
          headers: { authorization: `Bearer ${accessToken()}` },
        }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe rechazar un Bearer de refresco', () => {
      const next = vi.fn();
      const res = crearRes();
      const refresh = firmarToken(
        { sub: 'u-1', email: 'u@test', rol: ROLES.COMPRADOR, tipo: 'refresh' },
        SECRETO,
        '7d',
      );
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', { headers: { authorization: `Bearer ${refresh}` } }) as never,
        res as never,
        next,
      );
      expect(next).not.toHaveBeenCalled();
      expect(res.statusCode).toBe(401);
    });

    it('debe eximir del Basic al origen de Firebase configurado', () => {
      const next = vi.fn();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', {
          headers: { origin: 'https://kb-coleccion-internacional-ni.web.app' },
        }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe normalizar mayúsculas y slash final del origen exento', () => {
      const next = vi.fn();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', {
          headers: { origin: 'https://KB-Coleccion-Internacional-NI.web.app/' },
        }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe seguir exigiendo Basic a un origen no exento', () => {
      const next = vi.fn();
      const res = crearRes();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', { headers: { origin: 'https://otro.example' } }) as never,
        res as never,
        next,
      );
      expect(next).not.toHaveBeenCalled();
      expect(res.statusCode).toBe(401);
    });

    it('debe dejar /health abierto (liveness de contenedor)', () => {
      const next = vi.fn();
      edgeAuth(opciones)(crearReq('/health') as never, crearRes() as never, next);
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe dejar las imágenes públicas sin Basic (ADR-28)', () => {
      const next = vi.fn();
      edgeAuth(opciones)(
        crearReq('/api/v1/storage/public/objetos/22222222-2222-4222-8222-222222222222') as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });
  });

  describe('leerEdge', () => {
    it('debe aplicar valores por defecto seguros (diagnosticos privados, /ready privado, sin gate)', () => {
      const edge = leerEdge({} as NodeJS.ProcessEnv);
      expect(edge.basicAuth).toBeUndefined();
      expect(edge.bypassOrigins).toEqual([]);
      expect(edge.diagnosticosPublicos).toBe(false);
      expect(edge.readyPublico).toBe(false);
      expect(edge.jwtSecret).toBeUndefined();
    });

    it('debe leer el gate, la exposicion de diagnosticos y /ready del entorno', () => {
      const edge = leerEdge({
        EDGE_BASIC_AUTH: 'stage:clave',
        EDGE_BASIC_BYPASS_ORIGINS:
          'https://kb-coleccion-internacional-ni.web.app/, https://otro.example',
        DIAGNOSTICOS_PUBLICOS: 'true',
        READY_PUBLIC: 'true',
        INTERNAL_API_KEY: 'interna',
        JWT_SECRET: 'secreto-jwt',
      } as unknown as NodeJS.ProcessEnv);
      expect(edge.basicAuth).toBe('stage:clave');
      expect(edge.bypassOrigins).toEqual([
        'https://kb-coleccion-internacional-ni.web.app',
        'https://otro.example',
      ]);
      expect(edge.diagnosticosPublicos).toBe(true);
      expect(edge.readyPublico).toBe(true);
      expect(edge.claveInterna).toBe('interna');
      expect(edge.jwtSecret).toBe('secreto-jwt');
    });
  });
});
