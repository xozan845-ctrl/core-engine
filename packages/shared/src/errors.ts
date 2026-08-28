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
  constructor(mensaje: string, detalles?: unknown) {
    super('CONFLICTO', mensaje, detalles);
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

/** Status HTTP de cada error de dominio, segun la especificacion de la API. */
export function httpStatusDe(error: unknown): number {
  if (error instanceof NotFoundError) return 404;
  if (error instanceof UnauthorizedError) return 401;
  if (error instanceof ForbiddenError) return 403;
  if (error instanceof ConflictError) return 409;
  if (error instanceof ValidationError) return 400;
  if (error instanceof DomainError) return 400;
  return 500;
}