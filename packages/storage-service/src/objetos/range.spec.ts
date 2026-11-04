import { interpretarRango } from './range';

describe('interpretarRango (RFC 7233, un solo tramo)', () => {
  it('debe devolver completo cuando no hay cabecera', () => {
    expect(interpretarRango(undefined, 100)).toEqual({ tipo: 'completo' });
    expect(interpretarRango('   ', 100)).toEqual({ tipo: 'completo' });
  });

  it('debe interpretar bytes=inicio-fin, abierto y sufijo', () => {
    expect(interpretarRango('bytes=10-19', 100)).toEqual({
      tipo: 'parcial',
      rango: { inicio: 10, fin: 19 },
    });
    expect(interpretarRango('bytes=90-', 100)).toEqual({
      tipo: 'parcial',
      rango: { inicio: 90, fin: 99 },
    });
    expect(interpretarRango('bytes=-10', 100)).toEqual({
      tipo: 'parcial',
      rango: { inicio: 90, fin: 99 },
    });
  });

  it('debe recortar el fin al tamaño total y aceptar un sufijo mayor', () => {
    expect(interpretarRango('bytes=90-200', 100)).toEqual({
      tipo: 'parcial',
      rango: { inicio: 90, fin: 99 },
    });
    expect(interpretarRango('bytes=-200', 100)).toEqual({
      tipo: 'parcial',
      rango: { inicio: 0, fin: 99 },
    });
  });

  it('debe rechazar rangos inválidos, invertidos o fuera del objeto', () => {
    for (const cabecera of [
      'bytes=100-',
      'bytes=99-98',
      'bytes=-0',
      'bytes=',
      'bytes=0-1,5-9',
      'items=0-1',
      'bytes=abc-def',
    ]) {
      expect(interpretarRango(cabecera, 100)).toEqual({ tipo: 'invalido' });
    }
  });

  it('debe rechazar cualquier rango cuando el objeto está vacío', () => {
    expect(interpretarRango('bytes=0-0', 0)).toEqual({ tipo: 'invalido' });
    expect(interpretarRango(undefined, 0)).toEqual({ tipo: 'completo' });
  });
});
