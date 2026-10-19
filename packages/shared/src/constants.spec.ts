import {
  ROLES,
  ROLES_REGISTRABLES,
  ROLES_LOGISTICA,
  TENANT_HEADER,
  PERSONAL_HEADER,
  MARGEN_MINIMO,
  MARGEN_MAXIMO,
  COMMISSION_RATE_DEFAULT,
  CARRITO_EXPIRACION_MS,
  LIQUIDACION_DIAS,
  ESTADOS_VISIBLES_PUBLICO,
  JWT_ACCESS_TTL_DEFAULT,
  JWT_REFRESH_TTL_DEFAULT,
  PUERTOS,
  NOMBRE_SERVICIOS,
  STORAGE_BUCKET_PREFIX_DEFAULT,
  STORAGE_MAX_OBJETO_BYTES_DEFAULT,
  STORAGE_CUOTA_DEFAULT_BYTES,
  STORAGE_URL_TTL_SEGUNDOS_DEFAULT,
  STORAGE_CONTENT_TYPES_DEFAULT,
} from './constants';

describe('constants (reglas de negocio y configuracion)', () => {
  it('debe fijar el margen RN-01 en enteros de 0 a 90', () => {
    expect(MARGEN_MINIMO).toBe(0);
    expect(MARGEN_MAXIMO).toBe(90);
  });

  it('debe fijar la comision RN-04 en 12 % por defecto', () => {
    expect(COMMISSION_RATE_DEFAULT).toBe(0.12);
  });

  it('debe fijar la expiracion del carrito RN-05 en 30 minutos y la liquidacion RN-07 en los dias 1 y 15', () => {
    expect(CARRITO_EXPIRACION_MS).toBe(30 * 60 * 1000);
    expect(LIQUIDACION_DIAS).toEqual([1, 15]);
  });

  it('debe exponer los roles registrables y los de logistica', () => {
    expect(ROLES_REGISTRABLES).toEqual([ROLES.EMPRESARIO, ROLES.COMPRADOR]);
    expect(ROLES_LOGISTICA).toContain(ROLES.ADMIN);
    expect(ROLES_LOGISTICA).toContain(ROLES.LOGISTICA);
    expect(ROLES_LOGISTICA).toContain(ROLES.OPERATIVO);
  });

  it('debe fijar las cabeceras de contexto y los TTLs de JWT del gateway', () => {
    expect(TENANT_HEADER).toBe('x-tenant');
    expect(PERSONAL_HEADER).toBe('x-user-personal');
    expect(JWT_ACCESS_TTL_DEFAULT).toBe('900s');
    expect(JWT_REFRESH_TTL_DEFAULT).toBe('7d');
  });

  it('debe fijar el puerto de cada servicio', () => {
    expect(PUERTOS.GATEWAY).toBe(8080);
    expect(PUERTOS.IDENTITY).toBe(3001);
    expect(PUERTOS.CATALOG).toBe(3002);
    expect(PUERTOS.INTELLIGENCE).toBe(3009);
    expect(PUERTOS.STORAGE).toBe(3010);
  });

  it('debe fijar el nombre de workspace de cada servicio', () => {
    expect(NOMBRE_SERVICIOS.CATALOG).toBe('catalog-service');
    expect(NOMBRE_SERVICIOS.COMMISSIONS).toBe('commissions-service');
    expect(NOMBRE_SERVICIOS.INTELLIGENCE).toBe('market-intelligence-service');
    expect(NOMBRE_SERVICIOS.STORAGE).toBe('storage-service');
  });

  it('debe fijar los limites por defecto del almacenamiento de objetos', () => {
    expect(STORAGE_BUCKET_PREFIX_DEFAULT).toBe('t-');
    expect(STORAGE_MAX_OBJETO_BYTES_DEFAULT).toBe(25 * 1024 * 1024);
    expect(STORAGE_CUOTA_DEFAULT_BYTES).toBe(5 * 1024 * 1024 * 1024);
    expect(STORAGE_URL_TTL_SEGUNDOS_DEFAULT).toBe(900);
    expect([...STORAGE_CONTENT_TYPES_DEFAULT]).toContain('image/png');
  });

  it('debe marcar solo enviada y entregada como estados visibles al publico', () => {
    expect([...ESTADOS_VISIBLES_PUBLICO]).toEqual(['enviada', 'entregada']);
  });
});
