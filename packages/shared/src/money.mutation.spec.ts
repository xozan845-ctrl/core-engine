import { Money } from './money';
import { DomainError } from './errors';

function capturar(fn: () => unknown): DomainError | undefined {
  try {
    fn();
    return undefined;
  } catch (error) {
    return error as DomainError;
  }
}

describe('Money (mutantes dirigidos)', () => {
  it('parsear conserva la instancia si ya es Money', () => {
    const monto = Money.desdeCentavos(123);
    expect(Money.parsear(monto)).toBe(monto);
  });

  it('parsear acepta { centavos } y rechaza null/undefined', () => {
    expect(Money.parsear({ centavos: 123 }).centavos).toBe(123);
    expect(() => Money.parsear(null as unknown as string)).toThrow(
      /Formato de monto invalido/,
    );
    expect(() => Money.parsear(undefined as unknown as string)).toThrow(DomainError);
  });

  it('parsear normaliza espacios, coma, enteros y un decimal', () => {
    expect(Money.parsear(' 1 150,5 ').centavos).toBe(115_050);
    expect(Money.parsear('1.5').centavos).toBe(150);
    expect(Money.parsear(1150).centavos).toBe(115_000);
  });

  it('esPositivo distingue cero de positivo', () => {
    expect(Money.desdeCentavos(0).esPositivo()).toBe(false);
    expect(Money.desdeCentavos(1).esPositivo()).toBe(true);
  });

  it('aplicarMargen acepta los límites 0/90 y rechaza fuera de rango', () => {
    expect(Money.desdeCentavos(100).aplicarMargen(0).centavos).toBe(100);
    expect(Money.desdeCentavos(100).aplicarMargen(90).centavos).toBe(190);
    expect(() => Money.desdeCentavos(100).aplicarMargen(-1)).toThrow(
      /margen debe estar entre 0 y 90/,
    );
    expect(() => Money.desdeCentavos(100).aplicarMargen(91)).toThrow(/margen debe estar entre 0 y 90/);
  });

  it('comision acepta una tasa válida y rechaza 0, negativo, 1 y mayor', () => {
    expect(Money.desdeCentavos(100).comision(0.5).centavos).toBe(50);
    for (const tasa of [0, -0.1, 1, 1.1]) {
      expect(() => Money.desdeCentavos(100).comision(tasa)).toThrow(
        /tasa debe estar entre 0 y 1/,
      );
    }
  });

  it('rechaza formatos inválidos con el mensaje de dominio', () => {
    for (const invalido of ['', '1.999', '-1', 'abc', '1,2,3']) {
      expect(() => Money.parsear(invalido)).toThrow(/Formato de monto invalido/);
    }
  });

  it('rechaza centavos no enteros y negativos con mensaje y código', () => {
    expect(() => Money.desdeCentavos(1.5)).toThrow(/entero no negativo/);
    expect(() => Money.desdeCentavos(-1)).toThrow(/entero no negativo/);
    expect(capturar(() => Money.desdeCentavos(1.5))?.codigo).toBe('MONTO_INVALIDO');
    expect(capturar(() => Money.parsear('abc'))?.codigo).toBe('MONTO_INVALIDO');
    expect(capturar(() => Money.desdeCentavos(100).aplicarMargen(-1))?.codigo).toBe(
      'MARGEN_INVALIDO',
    );
    expect(capturar(() => Money.desdeCentavos(100).comision(0))?.codigo).toBe(
      'TASA_INVALIDA',
    );
  });

  it('normaliza -0 y serializa sin signo', () => {
    expect(Money.desdeCentavos(-0).centavos).toBe(0);
    expect(Object.is(Money.desdeCentavos(-0).centavos, -0)).toBe(false);
    expect(Money.desdeCentavos(0).string()).toBe('0.00');
    expect(Money.desdeCentavos(100).string()).toBe('1.00');
    expect(Money.desdeCentavos(123).toString()).toBe('1.23');
  });
});
