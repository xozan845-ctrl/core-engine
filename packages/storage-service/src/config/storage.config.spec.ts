import { leerConfig } from './storage.config';

const base = {
  MINIO_ROOT_USER: 'usuario',
  MINIO_ROOT_PASSWORD: 'secreto',
} as unknown as NodeJS.ProcessEnv;

describe('storage.config (limites y secretos por entorno, R-CD-7)', () => {
  it('no debe exigir secretos al leer la configuracion (los valida onModuleInit)', () => {
    const config = leerConfig({} as NodeJS.ProcessEnv);
    expect(config.accessKey).toBe('');
    expect(config.secretKey).toBe('');
    expect(config.endpoint).toBe('minio');
  });

  it('debe aplicar los valores por defecto de limites y whitelist', () => {
    const config = leerConfig(base);
    expect(config.endpoint).toBe('minio');
    expect(config.puerto).toBe(9000);
    expect(config.usarSsl).toBe(false);
    // sin MINIO_PUBLIC_* la firma usa el endpoint interno (mismo host)
    expect(config.publicEndpoint).toBe('minio');
    expect(config.publicPuerto).toBe(9000);
    expect(config.publicUsarSsl).toBe(false);
    expect(config.prefijoBucket).toBe('t-');
    expect(config.maxObjetoBytes).toBe(25 * 1024 * 1024);
    expect(config.cuotaDefaultBytes).toBe(5 * 1024 * 1024 * 1024);
    expect(config.urlTtlSegundos).toBe(900);
    expect(config.contentTypesPermitidos).toContain('image/png');
    expect(config.region).toBe('us-east-1');
  });

  it('debe leer los overrides del entorno (endpoint, limites y whitelist)', () => {
    const config = leerConfig({
      ...base,
      MINIO_ENDPOINT: 'almacen.interno',
      MINIO_PORT: '9100',
      MINIO_USE_SSL: 'true',
      STORAGE_BUCKET_PREFIX: 'org-',
      STORAGE_MAX_OBJETO_BYTES: '1048576',
      STORAGE_CUOTA_DEFAULT_BYTES: '2097152',
      STORAGE_URL_TTL_SEGUNDOS: '120',
      STORAGE_CONTENT_TYPES_PERMITIDOS: 'image/png, application/pdf',
      MINIO_PUBLIC_ENDPOINT: 'almacen.kbcoleccion.com',
      MINIO_PUBLIC_PORT: '443',
      MINIO_PUBLIC_USE_SSL: 'true',
    } as unknown as NodeJS.ProcessEnv);
    expect(config.endpoint).toBe('almacen.interno');
    expect(config.puerto).toBe(9100);
    expect(config.usarSsl).toBe(true);
    expect(config.publicEndpoint).toBe('almacen.kbcoleccion.com');
    expect(config.publicPuerto).toBe(443);
    expect(config.publicUsarSsl).toBe(true);
    expect(config.prefijoBucket).toBe('org-');
    expect(config.maxObjetoBytes).toBe(1048576);
    expect(config.cuotaDefaultBytes).toBe(2097152);
    expect(config.urlTtlSegundos).toBe(120);
    expect(config.contentTypesPermitidos).toEqual(['image/png', 'application/pdf']);
  });

  it('debe ignorar valores no numericos o no positivos', () => {
    const config = leerConfig({
      ...base,
      MINIO_PORT: 'no-es-un-puerto',
      STORAGE_MAX_OBJETO_BYTES: '0',
      STORAGE_URL_TTL_SEGUNDOS: '-5',
      STORAGE_CONTENT_TYPES_PERMITIDOS: '   ,  ',
    } as unknown as NodeJS.ProcessEnv);
    expect(config.puerto).toBe(9000);
    expect(config.maxObjetoBytes).toBe(25 * 1024 * 1024);
    expect(config.urlTtlSegundos).toBe(900);
    expect(config.contentTypesPermitidos).toContain('application/pdf');
  });
});
