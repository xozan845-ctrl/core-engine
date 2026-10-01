import { ESTADOS_ORDEN, puedeTransicionar, validarTransicion } from './order-state';
import type { EstadoOrden } from './order-state';
import { DomainError } from './errors';

describe('order-state (ciclo de vida de la orden, Tabla 13)', () => {
  it('debe permitir la cadena feliz cuando se avanza en orden', () => {
    expect(puedeTransicionar('creada', 'pagada')).toBe(true);
    expect(puedeTransicionar('pagada', 'en_preparacion')).toBe(true);
    expect(puedeTransicionar('en_preparacion', 'enviada')).toBe(true);
    expect(puedeTransicionar('enviada', 'entregada')).toBe(true);
  });

  it('debe permitir cancelar desde estados activos y devolver desde enviada o entregada', () => {
    for (const estado of ['creada', 'pagada', 'en_preparacion']) {
      expect(puedeTransicionar(estado as EstadoOrden, 'cancelada')).toBe(true);
    }
    expect(puedeTransicionar('enviada', 'devuelta')).toBe(true);
    expect(puedeTransicionar('entregada', 'devuelta')).toBe(true);
  });

  it('debe prohibir los estados terminales y los saltos de nivel', () => {
    expect(puedeTransicionar('cancelada', 'pagada')).toBe(false);
    expect(puedeTransicionar('devuelta', 'enviada')).toBe(false);
    expect(puedeTransicionar('creada', 'entregada')).toBe(false);
    expect(puedeTransicionar('pagada', 'enviada')).toBe(false);
  });

  it('debe lanzar DomainError TRANSICION_INVALIDA cuando el salto no es valido', () => {
    let error: DomainError | undefined;
    try {
      validarTransicion('creada', 'entregada');
    } catch (e) {
      error = e as DomainError;
    }
    expect(error?.codigo).toBe('TRANSICION_INVALIDA');
    expect(error?.toResponse().mensaje).toContain('creada');
    expect(error?.toResponse().mensaje).toContain('entregada');
  });

  it('debe listar los siete estados del contrato en orden', () => {
    expect(ESTADOS_ORDEN).toEqual([
      'creada',
      'pagada',
      'en_preparacion',
      'enviada',
      'entregada',
      'cancelada',
      'devuelta',
    ]);
  });
});