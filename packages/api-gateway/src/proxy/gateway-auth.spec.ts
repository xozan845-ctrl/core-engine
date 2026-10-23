import { GatewayAuthMiddleware, POLITICAS_RUTAS } from './gateway-auth.middleware';
import { firmarToken, ROLES, TENANT_HEADER, PERSONAL_HEADER, PARTICIPACION_HEADER } from '@core/shared';

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
    next: vi.fn(),
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

  it('logout sin token -> 401 (revoca el refresh; R-GW-4)', () => {
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/auth/logout', metodo: 'POST' });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/auth/logout', 'POST'));
    expect(res.statusCode).toBe(401);
    expect((res as unknown as { cuerpo: { codigo: string } }).cuerpo.codigo).toBe('NO_AUTORIZADO');
  });

  it('logout con token valido de cualquier rol -> pasa y propaga x-user-id', () => {
    const token = firmarToken(
      { sub: 'u-vend', email: 'v@t', rol: ROLES.VENDEDOR, tipo: 'access' },
      secreto,
      '900s',
    );
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/auth/logout', metodo: 'POST', token });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/auth/logout', 'POST'));
    expect(next).toHaveBeenCalled();
    expect((req.headers as Record<string, string>)['x-user-id']).toBe('u-vend');
  });
});

describe('Gateway auth — politicas de pedidos (R-GW-1, R-GW-3)', () => {
  const secreto = 'test_secret_jwt';
  const auth = new GatewayAuthMiddleware(secreto);
  const tokenDe = (rol: (typeof ROLES)[keyof typeof ROLES]) =>
    firmarToken({ sub: `u-${rol}`, email: `${rol}@test`, rol, tipo: 'access' }, secreto, '900s');

  function evaluar(ruta: string, metodo: string, rol: (typeof ROLES)[keyof typeof ROLES]) {
    const { req, res, next } = crearPeticion({ ruta, metodo, token: tokenDe(rol) });
    auth.aplicar(req as never, res as never, next, politicaDe(ruta, metodo));
    return { res: res as unknown as { statusCode: number; cuerpo?: { codigo: string } }, next };
  }

  it('debe denegar al comprador el listado global de ordenes cuando intenta GET /orders/admin/todas', () => {
    const { res, next } = evaluar('/api/v1/orders/admin/todas', 'GET', ROLES.COMPRADOR);
    expect(res.statusCode).toBe(403);
    expect(res.cuerpo?.codigo).toBe('ACCESO_DENEGADO');
    expect(next).not.toHaveBeenCalled();
  });

  it('debe denegar al vendedor el listado global de ordenes cuando intenta GET /orders/admin/todas', () => {
    const { res, next } = evaluar('/api/v1/orders/admin/todas', 'GET', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al admin el listado global de ordenes cuando intenta GET /orders/admin/todas', () => {
    const { res, next } = evaluar('/api/v1/orders/admin/todas', 'GET', ROLES.ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al super_admin el listado global de ordenes cuando intenta GET /orders/admin/todas', () => {
    const { res, next } = evaluar('/api/v1/orders/admin/todas', 'GET', ROLES.SUPER_ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al admin el listado de ordenes de una empresa cuando intenta GET /orders/empresa', () => {
    const { res, next } = evaluar('/api/v1/orders/empresa', 'GET', ROLES.ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al super_admin el detalle de una orden cuando intenta GET /orders/:id', () => {
    const { res, next } = evaluar('/api/v1/orders/orden-1', 'GET', ROLES.SUPER_ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al super_admin el timeline de una orden cuando intenta GET /orders/:id/timeline', () => {
    const { res, next } = evaluar('/api/v1/orders/orden-1/timeline', 'GET', ROLES.SUPER_ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al empresario el listado de ordenes de su empresa cuando intenta GET /orders/empresa', () => {
    const { res, next } = evaluar('/api/v1/orders/empresa', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al vendedor el listado de ordenes de la empresa cuando intenta GET /orders/empresa', () => {
    const { res, next } = evaluar('/api/v1/orders/empresa', 'GET', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe denegar al comprador las ordenes de una tienda cuando intenta GET /orders/tienda/:id', () => {
    const { res, next } = evaluar('/api/v1/orders/tienda/tienda-1', 'GET', ROLES.COMPRADOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al vendedor las ordenes de una tienda cuando intenta GET /orders/tienda/:id', () => {
    const { res, next } = evaluar('/api/v1/orders/tienda/tienda-1', 'GET', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al comprador las ventas del vendedor cuando intenta GET /orders/vendedor/mis-ventas', () => {
    const { res, next } = evaluar('/api/v1/orders/vendedor/mis-ventas', 'GET', ROLES.COMPRADOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe denegar al comprador las comisiones del vendedor cuando intenta GET /orders/vendedor/comisiones', () => {
    const { res, next } = evaluar('/api/v1/orders/vendedor/comisiones', 'GET', ROLES.COMPRADOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al comprador el detalle de una orden cuando intenta GET /orders/:id', () => {
    const { res, next } = evaluar('/api/v1/orders/orden-1', 'GET', ROLES.COMPRADOR);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al super_admin las rutas administrativas cuando intenta GET /admin/reportes', () => {
    const { res, next } = evaluar('/api/v1/admin/reportes', 'GET', ROLES.SUPER_ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al super_admin escribir en el catalogo cuando intenta POST /catalog/productos', () => {
    const { res, next } = evaluar('/api/v1/catalog/productos', 'POST', ROLES.SUPER_ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al empresario el detalle de una orden de su empresa cuando intenta GET /orders/:id', () => {
    const { res, next } = evaluar('/api/v1/orders/orden-1', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al empresario el timeline cuando intenta GET /orders/:id/timeline', () => {
    const { res, next } = evaluar('/api/v1/orders/orden-1/timeline', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al empresario las ordenes de una tienda cuando intenta GET /orders/tienda/:id', () => {
    const { res, next } = evaluar('/api/v1/orders/tienda/tienda-1', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al empresario avanzar el estado cuando intenta PATCH /orders/:id/estado', () => {
    const { res, next } = evaluar('/api/v1/orders/orden-1/estado', 'PATCH', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al empresario el listado global de ordenes cuando intenta GET /orders/admin/todas', () => {
    const { res, next } = evaluar('/api/v1/orders/admin/todas', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe denegar al empresario el estado de cuenta del vendedor cuando intenta GET /vendedores/me/ventas', () => {
    const { res, next } = evaluar('/api/v1/vendedores/me/ventas', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al empresario el estado de cuenta de su empresa cuando intenta GET /empresarios/me/ventas', () => {
    const { res, next } = evaluar('/api/v1/empresarios/me/ventas', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al vendedor el estado de cuenta de la empresa cuando intenta GET /empresarios/me/ventas', () => {
    const { res, next } = evaluar('/api/v1/empresarios/me/ventas', 'GET', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al vendedor su estado de cuenta cuando intenta GET /vendedores/me/ventas', () => {
    const { res, next } = evaluar('/api/v1/vendedores/me/ventas', 'GET', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al empresario listar sus tiendas cuando intenta GET /empresarios/me/tiendas', () => {
    const { res, next } = evaluar('/api/v1/empresarios/me/tiendas', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al vendedor las tiendas de la empresa cuando intenta GET /empresarios/me/tiendas', () => {
    const { res, next } = evaluar('/api/v1/empresarios/me/tiendas', 'GET', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al super_admin avanzar un envio de flota cuando intenta PATCH /envios/:id/estado', () => {
    const { res, next } = evaluar('/api/v1/envios/envio-1/estado', 'PATCH', ROLES.SUPER_ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al vendedor configurar el bucket cuando intenta POST /storage/bucket', () => {
    const { res, next } = evaluar('/api/v1/storage/bucket', 'POST', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al admin configurar el bucket cuando intenta POST /storage/bucket', () => {
    const { res, next } = evaluar('/api/v1/storage/bucket', 'POST', ROLES.ADMIN);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al vendedor crear vendedores de una empresa cuando intenta POST /empresas/:id/vendedores', () => {
    const { res, next } = evaluar('/api/v1/empresas/empresa-1/vendedores', 'POST', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al empresario crear vendedores de su empresa cuando intenta POST /empresas/:id/vendedores', () => {
    const { res, next } = evaluar('/api/v1/empresas/empresa-1/vendedores', 'POST', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al admin cambiar la logistica de una empresa cuando intenta PATCH /empresas/:id/logistica', () => {
    const { res, next } = evaluar('/api/v1/empresas/empresa-1/logistica', 'PATCH', ROLES.ADMIN);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe propagar los claims opcionales cuando el token trae nombre, tenant, personal y participacion', () => {
    const secreto = 'test_secret_jwt';
    const auth = new GatewayAuthMiddleware(secreto);
    const token = firmarToken(
      {
        sub: 'u-v',
        email: 'v@test',
        rol: ROLES.VENDEDOR,
        tipo: 'access',
        nombre: 'Ana',
        tenant_id: 'tenant-1',
        personal_id: 'p-1',
        participacion_porcentaje: 20,
      },
      secreto,
      '900s',
    );
    const { req, res, next } = crearPeticion({ ruta: '/api/v1/orders/orden-1', metodo: 'GET', token });
    auth.aplicar(req as never, res as never, next, politicaDe('/api/v1/orders/orden-1', 'GET'));
    expect(next).toHaveBeenCalled();
    const headers = req.headers as Record<string, string>;
    expect(headers['x-user-nombre']).toBe('Ana');
    expect(headers[TENANT_HEADER]).toBe('tenant-1');
    expect(headers[PERSONAL_HEADER]).toBe('p-1');
    expect(headers[PARTICIPACION_HEADER]).toBe('20');
  });
});

describe('Gateway auth — slugs de empresa y tienda (ADR-26)', () => {
  const secreto = 'test_secret_jwt';
  const auth = new GatewayAuthMiddleware(secreto);
  const tokenDe = (rol: (typeof ROLES)[keyof typeof ROLES]) =>
    firmarToken({ sub: `u-${rol}`, email: `${rol}@test`, rol, tipo: 'access' }, secreto, '900s');

  function evaluar(ruta: string, metodo: string, rol?: (typeof ROLES)[keyof typeof ROLES]) {
    const { req, res, next } = crearPeticion({
      ruta,
      metodo,
      token: rol ? tokenDe(rol) : null,
    });
    auth.aplicar(req as never, res as never, next, politicaDe(ruta, metodo));
    return { res: res as unknown as { statusCode: number; cuerpo?: { codigo: string } }, next };
  }

  it('debe exigir token para resolver una empresa por slug', () => {
    const { res, next } = evaluar('/api/v1/empresas/acme', 'GET');
    expect(res.statusCode).toBe(401);
    expect(res.cuerpo?.codigo).toBe('NO_AUTORIZADO');
    expect(next).not.toHaveBeenCalled();
  });

  it('debe denegar al comprador el slug de una empresa', () => {
    const { res, next } = evaluar('/api/v1/empresas/acme', 'GET', ROLES.COMPRADOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir al empresario resolver su empresa por slug', () => {
    const { res, next } = evaluar('/api/v1/empresas/acme', 'GET', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al empresario cambiar el slug de la empresa', () => {
    const { res, next } = evaluar('/api/v1/empresas/acme/slug', 'PATCH', ROLES.EMPRESARIO);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al vendedor cambiar el slug de la empresa', () => {
    const { res, next } = evaluar('/api/v1/empresas/acme/slug', 'PATCH', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });

  it('debe permitir la tienda pública por slug sin token', () => {
    const { res, next } = evaluar('/api/v1/tiendas/mi-tienda', 'GET');
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe permitir al vendedor cambiar el slug de su tienda', () => {
    const { res, next } = evaluar('/api/v1/tiendas/mi-tienda/slug', 'PATCH', ROLES.VENDEDOR);
    expect(res.statusCode).toBe(200);
    expect(next).toHaveBeenCalled();
  });

  it('debe denegar al comprador cambiar el slug de una tienda', () => {
    const { res, next } = evaluar('/api/v1/tiendas/mi-tienda/slug', 'PATCH', ROLES.COMPRADOR);
    expect(res.statusCode).toBe(403);
    expect(next).not.toHaveBeenCalled();
  });
});

describe('Gateway auth — deny by default (R-GW-1, R-GW-3)', () => {
  const secreto = 'test_secret_jwt';
  const auth = new GatewayAuthMiddleware(secreto);
  const token = firmarToken(
    { sub: 'u-comprador', email: 'comprador@test', rol: ROLES.COMPRADOR, tipo: 'access' },
    secreto,
    '900s',
  );

  it('debe denegar una ruta /api/v1 no declarada cuando el token es valido', () => {
    const ruta = '/api/v1/ruta-no-declarada';
    const { req, res, next } = crearPeticion({ ruta, metodo: 'GET', token });
    auth.aplicar(req as never, res as never, next, politicaDe(ruta, 'GET'));
    expect((res as unknown as { statusCode: number }).statusCode).toBe(403);
    expect((res as unknown as { cuerpo: { codigo: string } }).cuerpo.codigo).toBe('ACCESO_DENEGADO');
    expect(next).not.toHaveBeenCalled();
  });

  it('debe exigir token en una ruta /api/v1 no declarada cuando no hay token', () => {
    const ruta = '/api/v1/ruta-no-declarada';
    const { req, res, next } = crearPeticion({ ruta, metodo: 'GET' });
    auth.aplicar(req as never, res as never, next, politicaDe(ruta, 'GET'));
    expect((res as unknown as { statusCode: number }).statusCode).toBe(401);
    expect((res as unknown as { cuerpo: { codigo: string } }).cuerpo.codigo).toBe('NO_AUTORIZADO');
    expect(next).not.toHaveBeenCalled();
  });
});