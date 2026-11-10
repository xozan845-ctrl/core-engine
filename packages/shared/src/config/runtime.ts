/**
 * Configuración de runtime con defaults SOLO para desarrollo.
 *
 * En producción (`NODE_ENV=production`) un secreto ausente debe detener el
 * arranque, no degradarse a un valor conocido por cualquiera. Los defaults de
 * desarrollo se conservan para tests y `docker-compose.dev.yml`.
 */

type Entorno = Record<string, string | undefined>;

function esProduccion(env: Entorno): boolean {
  return (env.NODE_ENV ?? '').trim().toLowerCase() === 'production';
}

function requeridoEnProduccion(
  nombre: string,
  valor: string | undefined,
  defaultDesarrollo: string,
  env: Entorno,
): string {
  const limpio = valor?.trim();
  if (limpio) return limpio;
  if (esProduccion(env)) {
    throw new Error(`${nombre} es obligatoria en producción.`);
  }
  return defaultDesarrollo;
}

/** Secreto HS256 compartido por gateway e identity. */
export function secretoJwt(env: Entorno = process.env): string {
  return requeridoEnProduccion('JWT_SECRET', env.JWT_SECRET, 'dev_secret', env);
}

/** Password inicial del super_admin sembrado en el primer arranque. */
export function passwordAdminInicial(env: Entorno = process.env): string {
  return requeridoEnProduccion(
    'ADMIN_PASSWORD',
    env.ADMIN_PASSWORD,
    'AdminCore Engine2026!',
    env,
  );
}

/** Correo del super_admin inicial. */
export function emailAdminInicial(env: Entorno = process.env): string {
  return requeridoEnProduccion(
    'ADMIN_EMAIL',
    env.ADMIN_EMAIL,
    'admin@core-engine.test',
    env,
  );
}
