/**
 * Validación de secretos de arranque (R-DS-1 / R-CD-7).
 *
 * El objetivo es que un despliegue no arranque con secretos de plantilla, con
 * el fallback histórico `dev_secret` ni con valores demasiado cortos. En
 * desarrollo se permite cualquier valor presente para no romper el flujo
 * local, pero en producción se falla cerrado.
 */

/** Valores que sabemos que están versionados o que son de ejemplo. */
const PLACEHOLDER = /(cambiar_en_produccion|generar_con_npm|^<CAMBIAR|^changeme|^change_me|^dev_secret$|^dev_internal_key$)/i;

export function esProduccion(env: NodeJS.ProcessEnv = process.env): boolean {
  return (env.NODE_ENV ?? '').toLowerCase() === 'production';
}

/** Un secreto es inseguro si falta, es corto o es un placeholder conocido. */
export function esSecretoInseguro(valor: string | undefined): boolean {
  if (!valor) return true;
  const limpio = valor.trim();
  return limpio.length < 32 || PLACEHOLDER.test(limpio);
}

/**
 * Lee `JWT_SECRET`. No hay valor por defecto: si falta, el servicio no arranca.
 * En producción exige >= 32 caracteres y rechaza placeholders conocidos.
 */
export function leerSecretoJwt(env: NodeJS.ProcessEnv = process.env): string {
  const valor = env.JWT_SECRET?.trim();
  if (!valor) {
    throw new Error(
      'JWT_SECRET es obligatorio (no existe un valor por defecto). Genera uno con `npm run env:dev`.',
    );
  }
  if (esProduccion(env) && esSecretoInseguro(valor)) {
    throw new Error(
      'JWT_SECRET inseguro: usa al menos 32 caracteres aleatorios y no un valor de plantilla.',
    );
  }
  return valor;
}

/**
 * Lee `INTERNAL_API_KEY`. Igual que el JWT: obligatoria y fuerte en producción.
 */
export function leerClaveInterna(env: NodeJS.ProcessEnv = process.env): string {
  const valor = env.INTERNAL_API_KEY?.trim();
  if (!valor) {
    throw new Error(
      'INTERNAL_API_KEY es obligatoria (no existe un valor por defecto). Genera una con `npm run env:dev`.',
    );
  }
  if (esProduccion(env) && esSecretoInseguro(valor)) {
    throw new Error(
      'INTERNAL_API_KEY insegura: usa al menos 32 caracteres aleatorios y no un valor de plantilla.',
    );
  }
  return valor;
}
