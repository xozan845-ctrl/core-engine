import { DomainError } from './errors';

/**
 * Slugs legibles para empresas y tiendas (ADR-26).
 *
 * El slug es una clave pública estable para URLs; el UUID sigue siendo la
 * clave interna. La normalización es determinista y solo produce
 * `[a-z0-9-]`, sin acentos, espacios ni símbolos.
 */
export const SLUG_MIN_LENGTH = 2;
export const SLUG_MAX_LENGTH = 80;
export const SLUG_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

/** Palabras que colisionarían con rutas, roles o recursos del API. */
export const SLUGS_RESERVADOS: ReadonlySet<string> = new Set([
  'admin',
  'administrador',
  'api',
  'app',
  'auth',
  'carrito',
  'catalog',
  'docs',
  'empresa',
  'empresas',
  'empresario',
  'empresarios',
  'envios',
  'health',
  'internal',
  'login',
  'logout',
  'me',
  'metrics',
  'mis',
  'nuevo',
  'nueva',
  'ofertas',
  'orders',
  'pagos',
  'productos',
  'public',
  'ready',
  'registro',
  'reportes',
  'shop',
  'static',
  'tienda',
  'tiendas',
  'todas',
  'todos',
  'vendedor',
  'vendedores',
]);

// Cualquier UUID canonico (v1-v8 y variantes futuras) se trata como clave
// interna; la validacion de version no debe convertir una clave en slug.
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Un UUID se resuelve como clave interna, nunca como slug. */
export function esUuid(valor: string): boolean {
  return UUID_PATTERN.test(valor);
}

/**
 * Normaliza un texto a slug: quita diacríticos, baja a minúsculas, convierte
 * cualquier secuencia no alfanumérica en un guion y recorta a la longitud
 * máxima. No lanza: un texto sin caracteres útiles devuelve cadena vacía.
 */
export function normalizarSlug(valor: string): string {
  return valor
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, SLUG_MAX_LENGTH)
    .replace(/-+$/g, '');
}

/** Valida el formato ya normalizado y rechaza reservados y UUID. */
export function esSlugValido(valor: string): boolean {
  return (
    valor.length >= SLUG_MIN_LENGTH &&
    valor.length <= SLUG_MAX_LENGTH &&
    SLUG_PATTERN.test(valor) &&
    !SLUGS_RESERVADOS.has(valor) &&
    !esUuid(valor)
  );
}

/**
 * Deriva un slug base desde un nombre. Si el nombre no aporta caracteres
 * útiles, usa `fallback`; si queda demasiado corto, le añade un sufijo.
 * La colisión con otros recursos se resuelve fuera, con `slugConSufijo`.
 */
export function slugDesdeNombre(nombre: string, fallback = 'entidad'): string {
  let base = normalizarSlug(nombre);
  if (!base) base = normalizarSlug(fallback) || 'entidad';
  if (base.length < SLUG_MIN_LENGTH) base = normalizarSlug(`${base}-1`) || 'entidad-1';
  if (SLUGS_RESERVADOS.has(base)) base = slugConSufijo(base, 1);
  if (esUuid(base)) base = `${base}-x`;
  return base.slice(0, SLUG_MAX_LENGTH).replace(/-+$/g, '');
}

/**
 * Añade un sufijo numérico a un slug base sin superar la longitud máxima.
 * Se usa para resolver colisiones de forma determinista.
 */
export function slugConSufijo(base: string, numero: number): string {
  const sufijo = `-${numero}`;
  const maximoBase = Math.max(SLUG_MIN_LENGTH, SLUG_MAX_LENGTH - sufijo.length);
  const recorte = base.slice(0, maximoBase).replace(/-+$/g, '');
  return `${recorte || 'entidad'}${sufijo}`;
}

/**
 * Normaliza y valida un slug enviado por el cliente. Un valor inválido es un
 * error de dominio (400), no una excepción de infraestructura.
 */
export function validarSlugSolicitado(valor: string): string {
  const slug = normalizarSlug(valor);
  if (!slug || slug.length < SLUG_MIN_LENGTH) {
    throw new DomainError(
      'SLUG_INVALIDO',
      `El slug debe tener entre ${SLUG_MIN_LENGTH} y ${SLUG_MAX_LENGTH} caracteres alfanuméricos o guiones.`,
    );
  }
  if (SLUGS_RESERVADOS.has(slug)) {
    throw new DomainError('SLUG_RESERVADO', `El slug "${slug}" está reservado por la plataforma.`);
  }
  if (esUuid(slug)) {
    throw new DomainError('SLUG_INVALIDO', 'El slug no puede tener formato de UUID.');
  }
  if (!SLUG_PATTERN.test(slug) || slug.length > SLUG_MAX_LENGTH) {
    throw new DomainError('SLUG_INVALIDO', 'El slug solo admite letras minúsculas, números y guiones.');
  }
  return slug;
}
