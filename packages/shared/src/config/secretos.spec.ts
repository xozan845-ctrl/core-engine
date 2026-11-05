import { esProduccion, esSecretoInseguro, leerClaveInterna, leerSecretoJwt } from './secretos';

const FUERTE = 'a'.repeat(48);

describe('secretos de arranque', () => {
  it('detecta producción y secretos inseguros', () => {
    expect(esProduccion({ NODE_ENV: 'production' })).toBe(true);
    expect(esProduccion({ NODE_ENV: 'development' })).toBe(false);
    expect(esSecretoInseguro(undefined)).toBe(true);
    expect(esSecretoInseguro('corto')).toBe(true);
    expect(esSecretoInseguro('dev_secret_cambiar_en_produccion')).toBe(true);
    expect(esSecretoInseguro('<CAMBIAR_ESTO>')).toBe(true);
    expect(esSecretoInseguro('<GENERAR_CON_NPM_RUN_ENV_DEV>')).toBe(true);
    expect(esSecretoInseguro(FUERTE)).toBe(false);
  });

  it('exige JWT_SECRET y rechaza placeholders en producción', () => {
    expect(() => leerSecretoJwt({})).toThrow(/JWT_SECRET es obligatorio/);
    expect(() => leerSecretoJwt({ NODE_ENV: 'production', JWT_SECRET: 'dev_secret_cambiar_en_produccion' })).toThrow(
      /inseguro/,
    );
    expect(() => leerSecretoJwt({ NODE_ENV: 'production', JWT_SECRET: 'corto' })).toThrow(/inseguro/);
    expect(leerSecretoJwt({ NODE_ENV: 'production', JWT_SECRET: FUERTE })).toBe(FUERTE);
    // En desarrollo se permite un valor presente aunque sea de plantilla.
    expect(leerSecretoJwt({ NODE_ENV: 'development', JWT_SECRET: 'dev_secret_cambiar_en_produccion' })).toBe(
      'dev_secret_cambiar_en_produccion',
    );
  });

  it('exige INTERNAL_API_KEY y rechaza placeholders en producción', () => {
    expect(() => leerClaveInterna({})).toThrow(/INTERNAL_API_KEY es obligatoria/);
    expect(() =>
      leerClaveInterna({ NODE_ENV: 'production', INTERNAL_API_KEY: 'dev_internal_key_cambiar_en_produccion' }),
    ).toThrow(/insegura/);
    expect(leerClaveInterna({ NODE_ENV: 'production', INTERNAL_API_KEY: FUERTE })).toBe(FUERTE);
  });
});
