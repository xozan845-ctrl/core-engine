import fc from 'fast-check';
import { ESTADOS_ORDEN, validarTransicionPorTipo } from './order-state';
import type { EstadoOrden } from './order-state';
import { DomainError } from './errors';

const estado = fc.constantFrom<EstadoOrden>(...ESTADOS_ORDEN);

function puedeTransicionar(de: EstadoOrden, a: EstadoOrden): boolean {
  try {
    validarTransicionPorTipo('compra', de, a);
    return true;
  } catch {
    return false;
  }
}

describe('order-state (propiedades)', () => {
  it('debe prohibir la transición de un estado a sí mismo', () => {
    fc.assert(
      fc.property(estado, (actual) => {
        expect(puedeTransicionar(actual, actual)).toBe(false);
      }),
    );
  });

  it('debe dejar sin salida a los estados terminales', () => {
    for (const terminal of ['cancelada', 'devuelta'] as const) {
      for (const destino of ESTADOS_ORDEN) {
        expect(puedeTransicionar(terminal, destino)).toBe(false);
      }
    }
  });

  it('debe aceptar solo destinos declarados en el contrato', () => {
    fc.assert(
      fc.property(estado, estado, (desde, hacia) => {
        if (puedeTransicionar(desde, hacia)) {
          expect(ESTADOS_ORDEN).toContain(hacia);
        }
      }),
    );
  });

  it('validarTransicionPorTipo debe lanzar exactamente cuando la transición no es válida', () => {
    fc.assert(
      fc.property(estado, estado, (desde, hacia) => {
        if (puedeTransicionar(desde, hacia)) {
          expect(() => validarTransicionPorTipo('compra', desde, hacia)).not.toThrow();
        } else {
          expect(() => validarTransicionPorTipo('compra', desde, hacia)).toThrow(DomainError);
        }
      }),
    );
  });
});
