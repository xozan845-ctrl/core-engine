import {
  firmarToken,
  verificarToken,
  crearAccessToken,
  crearRefreshToken,
  esRol,
  usuarioDesdeHeaders,
} from './jwt.utils';
import type { TokenPayload } from './jwt.utils';
import { ROLES } from '../constants';

describe('jwt.utils (firma y verificacion HS256, doc 4.3)', () => {
  const secreto = 'secreto-de-prueba';
  const payload: TokenPayload = {
    sub: 'u-1',
    email: 'vendedor@core.local',
    rol: ROLES.VENDEDOR,
    tipo: 'access',
    nombre: 'Ana',
    tenant_id: 't-1',
  };

  it('debe firmar y verificar un token sin mutar el payload cuando el secreto coincide', () => {
    const token = firmarToken(payload, secreto, '900s');
    expect(typeof token).toBe('string');
    const decodificado = verificarToken<TokenPayload>(token, secreto);
    expect(decodificado.sub).toBe('u-1');
    expect(decodificado.email).toBe('vendedor@core.local');
    expect(decodificado.rol).toBe(ROLES.VENDEDOR);
    expect(decodificado.tipo).toBe('access');
  });

  it('debe rechazar el token cuando el secreto no coincide', () => {
    const token = firmarToken(payload, secreto, '900s');
    expect(() => verificarToken(token, 'otro-secreto')).toThrow();
  });

  it('debe crear access y refresh con tipo y expiracion propios', () => {
    const usuario = {
      id: 'u-1',
      email: 'vendedor@core.local',
      rol: ROLES.VENDEDOR,
      nombre: 'Ana',
      tenant_id: 't-1',
    };
    const access = crearAccessToken(usuario, secreto, '900s');
    const refresh = crearRefreshToken(usuario, secreto, '7d');
    expect(verificarToken<TokenPayload>(access, secreto).tipo).toBe('access');
    expect(verificarToken<TokenPayload>(refresh, secreto).tipo).toBe('refresh');
    expect(verificarToken<TokenPayload>(access, secreto).sub).toBe('u-1');
  });

  it('debe firmar con HS256 y expiracion cuando se usa un ttl valido', () => {
    const token = firmarToken(payload, secreto, '1h');
    const claims = verificarToken<TokenPayload & { exp: number; iat: number }>(token, secreto);
    expect(claims.exp).toBeGreaterThan(claims.iat);
  });
});

describe('jwt.utils (roles)', () => {
  it('debe reconocer como rol cada valor de ROLES', () => {
    for (const rol of Object.values(ROLES)) {
      expect(esRol(rol)).toBe(true);
    }
  });

  it('debe rechazar un valor que no es rol', () => {
    expect(esRol('super-user')).toBe(false);
    expect(esRol('')).toBe(false);
    expect(esRol('admin ')).toBe(false);
  });
});

describe('jwt.utils (contexto desde cabeceras del gateway)', () => {
  it('debe construir el contexto cuando las cabeceras X-User-* estan completas', () => {
    const usuario = usuarioDesdeHeaders({
      'x-user-id': 'u-7',
      'x-user-email': 'comprador@core.local',
      'x-user-rol': ROLES.COMPRADOR,
      'x-tenant': 't-2',
      'x-user-nombre': 'Luis',
    });
    expect(usuario?.user_id).toBe('u-7');
    expect(usuario?.email).toBe('comprador@core.local');
    expect(usuario?.rol).toBe(ROLES.COMPRADOR);
    expect(usuario?.tenant_id).toBe('t-2');
    expect(usuario?.nombre).toBe('Luis');
  });

  it('debe devolver null cuando falta user-id o el rol no es valido', () => {
    expect(usuarioDesdeHeaders({ 'x-user-rol': ROLES.ADMIN })).toBeNull();
    expect(usuarioDesdeHeaders({ 'x-user-id': 'u-1', 'x-user-rol': 'roto' })).toBeNull();
  });

  it('debe tomar el primer valor cuando la cabecera llega como array', () => {
    const usuario = usuarioDesdeHeaders({ 'x-user-id': ['u-1', 'u-2'], 'x-user-rol': ROLES.VENDEDOR });
    expect(usuario?.user_id).toBe('u-1');
  });
});