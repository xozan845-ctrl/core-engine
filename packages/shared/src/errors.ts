/**
 * Errores estructurados de la API (doc: errores con campos {codigo, mensaje, detalles}).
 */
export class DomainError extends Error {
  readonly codigo: string;
  readonly detalles?: unknown;

  constructor(codigo: string, mensaje: string, detalles?: unknown) {
    super(mensaje);
    this.name = 'DomainError';
    this.codigo = codigo;
    this.detalles = detalles;
  }

  toResponse(): { codigo: string; mensaje: string; detalles?: unknown } {
    return { codigo: this.codigo, mensaje: this.message, detalles: this.detalles };
  }
}

export class NotFoundError extends DomainError {
  constructor(entidad: string, id: string) {
    super('NO_ENCONTRADO', `${entidad} ${id} no existe.`);
  }
}

export class ConflictError extends DomainError {
  constructor(mensaje: string, detalles?: unknown, codigo = 'CONFLICTO') {
    super(codigo, mensaje, detalles);
  }
}

/** Recurso que existió pero ya no es válido (token expirado, usado o revocado). */
export class GoneError extends DomainError {
  constructor(codigo: string, mensaje: string, detalles?: unknown) {
    super(codigo, mensaje, detalles);
  }
}

export class UnauthorizedError extends DomainError {
  constructor(mensaje = 'No autorizado.') {
    super('NO_AUTORIZADO', mensaje);
  }
}

export class ForbiddenError extends DomainError {
  constructor(mensaje = 'Acceso denegado.') {
    super('ACCESO_DENEGADO', mensaje);
  }
}

export class ValidationError extends DomainError {
  constructor(detalles?: unknown) {
    super('VALIDACION', 'La solicitud no cumple las reglas de validacion.', detalles);
  }
}

/**
 * `true` si PostgreSQL rechazo una escritura por una restriccion UNIQUE
 * (SQLSTATE 23505). Los servicios lo traducen a un error de dominio 409.
 */
export function esViolacionUnicidad(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    (error as { code?: unknown }).code === '23505'
  );
}

/** Status HTTP de cada error de dominio, segun la especificacion de la API. */
export function httpStatusDe(error: unknown): number {
  if (error instanceof NotFoundError) return 404;
  if (error instanceof UnauthorizedError) return 401;
  if (error instanceof ForbiddenError) return 403;
  if (error instanceof ConflictError) return 409;
  if (error instanceof GoneError) return 410;
  if (error instanceof ValidationError) return 400;
  if (error instanceof DomainError) return 400;
  return 500;
}