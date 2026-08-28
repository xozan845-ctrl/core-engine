import { GatewayAuthMiddleware, POLITICAS_RUTAS } from './gateway-auth.middleware';
import { firmarToken, ROLES } from '@core/shared';

function crearPeticion(config: {
  ruta: string;
  metodo: string;
  token?: string | null;
}): { req: Record<string, unknown>; res: Record<string, unknown>; next: () => void } {
  const res: Record<string, unknown> = {
    statusCode: 200,
    headers: {},
    status(codigo: number) {
      (this as Record<string, unknown>).statusCode = codigo;
      return this;
    },
    json(cuerpo: unknown) {
      (this as Record<string, unknown>).cuerpo = cuerpo;
      return this;
    },
    setHeader(k: string, v: string) {
      (this.headers as Record<string, string>)[k] = v;
    },
  };
  return {
    req: {
      originalUrl: config.ruta,
      method: config.metodo,
      headers: config.token
        ? { authorization: `Bearer ${config.token}` }
        : {},
    },
    res,
    next: jest.fn(),
  };
}

function politicaDe(ruta: string, metodo: string) {
  return (
    POLITICAS_RUTAS.find(
      (p) => p.patron.test(ruta) && (p.metodos.includes(metodo) || p.metodos.includes('*')),
    ) ?? null
  );
}

describe('Gateway auth (TC-07: seguridad en el borde)', () => {
  const secreto = 'test_secret_jwt';
  const auth = new GatewayAuthMiddleware(secreto);

  it('TC-07a: GET /api/v1/admin/reportes sin token -> 401', () => {
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/admin/reportes', metodo: 'GET' });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/admin/reportes', 'GET'));
    expect(res.statusCode).toBe(401);
    expect((res as unknown as { cuerpo: { codigo: string } }).cuerpo.codigo).toBe('NO_AUTORIZADO');
  });

  it('comprador sin permiso de admin -> 403', () => {
    const token = firmarToken(
      { sub: 'u1', email: 'comprador@test', rol: ROLES.COMPRADOR, tipo: 'access' },
      secreto,
      '900s',
    );
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/admin/reportes', metodo: 'GET', token });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/admin/reportes', 'GET'));
    expect(res.statusCode).toBe(403);
    expect((res as unknown as { cuerpo: { codigo: string } }).cuerpo.codigo).toBe('ACCESO_DENEGADO');
  });

  it('token invalido -> 401', () => {
    const { req, res, next } = crearPeticion({
      ruta: '/api/v1/orders/abc',
      metodo: 'GET',
      token: 'token-invalido',
    });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/orders/abc', 'GET'));
    expect(res.statusCode).toBe(401);
  });

  it('admin con token valido -> paso y cabeceras X-User-* propagadas', () => {
    const token = firmarToken(
      { sub: 'u-admin', email: 'admin@test', rol: ROLES.ADMIN, tipo: 'access' },
      secreto,
      '900s',
    );
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/admin/reportes', metodo: 'GET', token });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/admin/reportes', 'GET'));
    expect(next).toHaveBeenCalled();
    expect((req.headers as Record<string, string>)['x-user-rol']).toBe('admin');
    expect((req.headers as Record<string, string>)['x-user-id']).toBe('u-admin');
  });

  it('ruta publica (login) pasa sin token', () => {
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/auth/login', metodo: 'POST' });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/auth/login', 'POST'));
    expect(next).toHaveBeenCalled();
  });

  it('refresh token no sirve como access -> 401', () => {
    const token = firmarToken(
      { sub: 'u1', email: 'c@t', rol: ROLES.COMPRADOR, tipo: 'refresh' },
      secreto,
      '7d',
    );
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/orders', metodo: 'POST', token });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/orders', 'POST'));
    expect(res.statusCode).toBe(401);
  });
});