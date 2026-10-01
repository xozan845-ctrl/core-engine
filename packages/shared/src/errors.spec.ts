import {
  DomainError,
  NotFoundError,
  ConflictError,
  UnauthorizedError,
  ForbiddenError,
  ValidationError,
  httpStatusDe,
} from './errors';

describe('errors (errores estructurados de la API, doc 5.7)', () => {
  it('debe conservar codigo y mensaje en DomainError y exponerlos en toResponse', () => {
    const error = new DomainError('X', 'mensaje');
    expect(error.message).toBe('mensaje');
    expect(error.name).toBe('DomainError');
    expect(error.codigo).toBe('X');
    expect(error.toResponse()).toEqual({ codigo: 'X', mensaje: 'mensaje' });
  });

  it('debe incluir detalles en toResponse cuando existen', () => {
    const error = new DomainError('X', 'mensaje', { campo: 'a' });
    expect(error.toResponse()).toEqual({ codigo: 'X', mensaje: 'mensaje', detalles: { campo: 'a' } });
  });

  it('debe mapear cada error tipado a su status HTTP', () => {
    expect(httpStatusDe(new NotFoundError('Producto', 'p-1'))).toBe(404);
    expect(httpStatusDe(new UnauthorizedError())).toBe(401);
    expect(httpStatusDe(new ForbiddenError())).toBe(403);
    expect(httpStatusDe(new ConflictError('conflicto'))).toBe(409);
    expect(httpStatusDe(new ValidationError())).toBe(400);
    expect(httpStatusDe(new DomainError('X', 'm'))).toBe(400);
    expect(httpStatusDe(new Error('inesperado'))).toBe(500);
  });

  it('debe componer los mensajes y codigos de los errores tipados', () => {
    expect(new NotFoundError('Producto', 'p-1').message).toBe('Producto p-1 no existe.');
    expect(new UnauthorizedError().codigo).toBe('NO_AUTORIZADO');
    expect(new ForbiddenError().codigo).toBe('ACCESO_DENEGADO');
    expect(new ConflictError('c').codigo).toBe('CONFLICTO');
    expect(new ValidationError().codigo).toBe('VALIDACION');
    expect(new ValidationError('detalle').detalles).toBe('detalle');
  });
});