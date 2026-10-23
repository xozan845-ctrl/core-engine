import fc from 'fast-check';
import { Money } from './money';
import { DomainError } from './errors';

const centavos = fc.integer({ min: 0, max: 1_000_000_000 });
const centavosPositivos = centavos;

describe('Money (propiedades)', () => {
  it('debe conservar el valor cuando string() y parsear() hacen round-trip', () => {
    fc.assert(
      fc.property(centavos, (valor) => {
        const original = Money.desdeCentavos(valor);
        expect(Money.parsear(original.string()).centavos).toBe(valor);
      }),
    );
  });

  it('debe deshacer la suma con la resta', () => {
    fc.assert(
      fc.property(centavos, centavos, (a, b) => {
        const izquierda = Money.desdeCentavos(a);
        const derecha = Money.desdeCentavos(b);
        expect(izquierda.sumar(derecha).restar(derecha).centavos).toBe(a);
      }),
    );
  });

  it('debe conservar el monto con margen 0 y no reducirlo con margen positivo', () => {
    fc.assert(
      fc.property(centavosPositivos, fc.integer({ min: 0, max: 90 }), (valor, margen) => {
        const original = Money.desdeCentavos(valor);
        const final = original.aplicarMargen(margen);
        expect(final.centavos).toBeGreaterThanOrEqual(valor);
        if (margen === 0) expect(final.centavos).toBe(valor);
      }),
    );
  });

  it('debe acotar la comisión entre cero y el monto de venta', () => {
    fc.assert(
      fc.property(
        centavosPositivos,
        fc.double({ min: 0.001, max: 0.999, noNaN: true }),
        (valor, tasa) => {
          const comision = Money.desdeCentavos(valor).comision(tasa);
          expect(Number.isSafeInteger(comision.centavos)).toBe(true);
          expect(comision.centavos).toBeGreaterThanOrEqual(0);
          expect(comision.centavos).toBeLessThanOrEqual(valor);
        },
      ),
    );
  });

  it('debe conservar el monto al multiplicar por 1 y anularlo por 0', () => {
    fc.assert(
      fc.property(centavos, (valor) => {
        const original = Money.desdeCentavos(valor);
        expect(original.multiplicarPor(1).centavos).toBe(valor);
        expect(original.multiplicarPor(0).centavos).toBe(0);
      }),
    );
  });

  it('debe serializar siempre con exactamente dos decimales', () => {
    fc.assert(
      fc.property(centavos, (valor) => {
        expect(Money.desdeCentavos(valor).string()).toMatch(/^-?\d+\.\d{2}$/);
      }),
    );
  });

  it('debe rechazar cualquier formato de monto inválido', () => {
    const invalidos = fc.string().filter((texto) => {
      const normalizado = texto.trim().replace(',', '.').replace(/\s/g, '');
      return !/^\d+(\.\d{1,2})?$/.test(normalizado);
    });

    fc.assert(
      fc.property(invalidos, (texto) => {
        expect(() => Money.parsear(texto)).toThrow(DomainError);
      }),
    );
  });

  it('debe rechazar centavos no enteros, negativos o fuera del rango seguro', () => {
    expect(() => Money.desdeCentavos(1.5)).toThrow(DomainError);
    expect(() => Money.desdeCentavos(-1)).toThrow(DomainError);
    expect(() => Money.desdeCentavos(Number.MAX_SAFE_INTEGER + 1)).toThrow(DomainError);
  });
});
