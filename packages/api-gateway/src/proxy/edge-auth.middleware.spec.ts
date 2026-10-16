import { edgeAuth, leerEdge } from './edge-auth.middleware';

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

describe('edgeAuth (endurecimiento del borde: gate de entornos y diagnostico)', () => {
  describe('sin gate de Basic Auth', () => {
    it('debe dejar pasar todo cuando los diagnosticos son publicos (dev)', () => {
      const next = vi.fn();
      edgeAuth({ diagnosticosPublicos: true })(crearReq('/metrics') as never, crearRes() as never, next);
      edgeAuth({ diagnosticosPublicos: true })(crearReq('/docs') as never, crearRes() as never, next);
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

  describe('con gate de Basic Auth (staging)', () => {
    const opciones = { basicAuth: 'stage:secreto-e2e', diagnosticosPublicos: false };

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
        crearReq('/api/v1/orders', { headers: { authorization: basic('stage:secreto-e2e') } }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe dejar pasar un Bearer (el token lo evalua la capa JWT) sin exigir Basic', () => {
      const next = vi.fn();
      edgeAuth(opciones)(
        crearReq('/api/v1/orders', { headers: { authorization: 'Bearer token-de-sesion' } }) as never,
        crearRes() as never,
        next,
      );
      expect(next).toHaveBeenCalledTimes(1);
    });

    it('debe dejar /health abierto (liveness de contenedor)', () => {
      const next = vi.fn();
      edgeAuth(opciones)(crearReq('/health') as never, crearRes() as never, next);
      expect(next).toHaveBeenCalledTimes(1);
    });
  });

  describe('leerEdge', () => {
    it('debe aplicar valores por defecto seguros (diagnosticos privados, sin gate)', () => {
      const edge = leerEdge({} as NodeJS.ProcessEnv);
      expect(edge.basicAuth).toBeUndefined();
      expect(edge.diagnosticosPublicos).toBe(false);
    });

    it('debe leer el gate y la exposicion de diagnosticos del entorno', () => {
      const edge = leerEdge({
        EDGE_BASIC_AUTH: 'stage:clave',
        DIAGNOSTICOS_PUBLICOS: 'true',
        INTERNAL_API_KEY: 'interna',
      } as unknown as NodeJS.ProcessEnv);
      expect(edge.basicAuth).toBe('stage:clave');
      expect(edge.diagnosticosPublicos).toBe(true);
      expect(edge.claveInterna).toBe('interna');
    });
  });
});
