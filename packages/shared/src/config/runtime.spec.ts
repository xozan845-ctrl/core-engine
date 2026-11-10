import {
  secretoJwt,
  passwordAdminInicial,
  emailAdminInicial,
} from './runtime';

describe('runtime config (secretos obligatorios en producción)', () => {
  it('debe usar el valor de entorno cuando existe', () => {
    expect(secretoJwt({ NODE_ENV: 'production', JWT_SECRET: 's3creto' })).toBe('s3creto');
    expect(passwordAdminInicial({ NODE_ENV: 'production', ADMIN_PASSWORD: 'p4ss' })).toBe('p4ss');
    expect(emailAdminInicial({ NODE_ENV: 'production', ADMIN_EMAIL: 'a@b.c' })).toBe('a@b.c');
  });

  it('debe conservar los defaults de desarrollo fuera de producción', () => {
    expect(secretoJwt({ NODE_ENV: 'test' })).toBe('dev_secret');
    expect(passwordAdminInicial({ NODE_ENV: 'development' })).toBe('AdminCore Engine2026!');
    expect(emailAdminInicial({ NODE_ENV: 'development' })).toBe('admin@core-engine.test');
  });

  it('debe fallar en producción si el secreto JWT falta o está vacío', () => {
    expect(() => secretoJwt({ NODE_ENV: 'production' })).toThrow(/JWT_SECRET/);
    expect(() => secretoJwt({ NODE_ENV: 'production', JWT_SECRET: '   ' })).toThrow(/JWT_SECRET/);
  });

  it('debe fallar en producción si faltan las credenciales del admin', () => {
    expect(() => passwordAdminInicial({ NODE_ENV: 'production' })).toThrow(/ADMIN_PASSWORD/);
    expect(() => emailAdminInicial({ NODE_ENV: 'production' })).toThrow(/ADMIN_EMAIL/);
  });
});
