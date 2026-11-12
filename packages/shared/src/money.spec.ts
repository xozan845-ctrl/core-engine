import { Money } from './money';
import { DomainError } from './errors';

describe('Money (RN-01 y RN-04, ejemplo Tabla 5)', () => {
  it('precio base C$ 1 000 con margen 15 % = C$ 1 150,00 (RN-01)', () => {
    const base = Money.parsear('1000.00');
    expect(base.aplicarMargen(15).string()).toBe('1150.00');
  });

  it('comision 12 % de C$ 1 150 = C$ 138,00 y liquida C$ 1 012,00 (RN-04)', () => {
    const venta = Money.parsear('1150.00');
    const comision = venta.comision(0.12);
    expect(comision.string()).toBe('138.00');
    expect(venta.restar(comision).string()).toBe('1012.00');
  });

  it('acepta montos con coma y sin decimales', () => {
    expect(Money.parsear('1 150,50').string()).toBe('1150.50');
    expect(Money.parsear('450').string()).toBe('450.00');
    expect(Money.parsear(1150).string()).toBe('1150.00');
  });

  it('rechaza montos negativos (R-PB-3: centavos enteros y >= 0)', () => {
    expect(() => Money.parsear('-0.01')).toThrow(DomainError);
    expect(() => Money.desdeCentavos(-1)).toThrow(DomainError);
  });

  it('rechaza montos con mas de 2 decimales', () => {
    expect(() => Money.parsear('1.999')).toThrow(DomainError);
  });

  it('nunca usa punto flotante: 0.1 + 0.2 tipo centavos exactos', () => {
    const a = Money.parsear('0.10');
    const b = Money.parsear('0.20');
    expect(a.sumar(b).centavos).toBe(30);
  });
});
