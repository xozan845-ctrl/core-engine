import { DomainError } from './errors';

/**
 * Manejo de montos en cordobas (C$) con precision decimal como enteros
 * (centavos). Control OWASP A02: los montos nunca se manipulan en punto flotante.
 */
export class Money {
  readonly centavos: number;

  private constructor(centavos: number) {
    if (!Number.isSafeInteger(centavos)) {
      throw new DomainError('MONTO_INVALIDO', 'El monto debe ser un entero en centavos.');
    }
    this.centavos = centavos;
  }

  static desdeCentavos(centavos: number): Money {
    return new Money(centavos);
  }

  /** Convierte un monto decimal ("1150.00", 1150 o "1150,50") a centavos enteros. */
  static parsear(valor: string | number | { centavos: number } | Money): Money {
    if (valor instanceof Money) return valor;
    if (typeof valor === 'object' && valor !== null && 'centavos' in valor) {
      return new Money(valor.centavos);
    }
    const texto = String(valor).trim().replace(',', '.').replace(/\s/g, '');
    if (!/^\d+(\.\d{1,2})?$/.test(texto)) {
      throw new DomainError(
        'MONTO_INVALIDO',
        `Formato de monto invalido: "${valor}". Use numero con hasta 2 decimales.`,
      );
    }
    const partes = texto.split('.');
    const enteros = partes[0];
    const decimales = (partes[1] ?? '').padEnd(2, '0');
    return new Money(parseInt(enteros, 10) * 100 + parseInt(decimales || '0', 10));
  }

  esPositivo(): boolean {
    return this.centavos > 0;
  }

  sumar(otro: Money): Money {
    return new Money(this.centavos + otro.centavos);
  }

  restar(otro: Money): Money {
    return new Money(this.centavos - otro.centavos);
  }

  multiplicarPor(ratio: number): Money {
    return Money.desdeCentavos(Math.round(this.centavos * ratio));
  }

  /** RN-01: precio final = base x (1 + margen). El margen llega en enteros (15 = 15 %). */
  aplicarMargen(margenPorcentaje: number): Money {
    if (margenPorcentaje < 0 || margenPorcentaje > 90) {
      throw new DomainError('MARGEN_INVALIDO', 'El margen debe estar entre 0 y 90 % (RN-01).');
    }
    return Money.desdeCentavos(
      Math.round(this.centavos + (this.centavos * margenPorcentaje) / 100),
    );
  }

  /** RN-04: comision = precio de venta x tasa. */
  comision(tasa: number): Money {
    if (tasa <= 0 || tasa >= 1) {
      throw new DomainError('TASA_INVALIDA', `La tasa debe estar entre 0 y 1 (recibida: ${tasa}).`);
    }
    return Money.desdeCentavos(Math.round(this.centavos * tasa));
  }

  /** Serializacion "1150.00" (doc: montos en C$ con dos decimales). */
  string(): string {
    const signo = this.centavos < 0 ? '-' : '';
    const abs = Math.abs(this.centavos);
    const enteros = String(Math.floor(abs / 100));
    const decimales = String(abs % 100).padStart(2, '0');
    return `${signo}${enteros}.${decimales}`;
  }

  toString(): string {
    return this.string();
  }
}

export type MontoInput = string | number | { centavos: number } | Money;