import { DomainError } from '../errors';
import {
  TEMA_POR_DEFECTO,
  esFuente,
  esModo,
  esPlantilla,
  generarTemaCss,
  mezclarTema,
  normalizarTemaParcial,
  plantillaDe,
  resolverPersonalizacion,
  temaDe,
  temaParcialDe,
  validarTema,
} from './tema';

describe('tema compartido (ADR-39/ADR-42)', () => {
  it('filtra y normaliza un tema parcial', () => {
    expect(
      temaParcialDe({
        color_primario: ' #abc ',
        fuente: 'poppins',
        radio_bordes: 8,
        modo: 'oscuro',
        extra: 1,
      }),
    ).toEqual({
      color_primario: '#ABC',
      fuente: 'poppins',
      radio_bordes: 8,
      modo: 'oscuro',
    });
    expect(temaParcialDe(null)).toEqual({});
    expect(
      temaParcialDe({ color_primario: 'rojo', radio_bordes: 40, fuente: 'x' }),
    ).toEqual({});
  });

  it('mezcla capas con prioridad sobre los defaults', () => {
    const tema = mezclarTema(
      { color_primario: '#111111' },
      { color_primario: '#222222', fuente: 'inter' },
      { color_secundario: '#333333' },
    );

    expect(tema.color_primario).toBe('#222222');
    expect(tema.fuente).toBe('inter');
    expect(tema.color_secundario).toBe('#333333');
    expect(tema.color_fondo).toBe(TEMA_POR_DEFECTO.color_fondo);
  });

  it('normaliza un update parcial sin rellenar defaults', () => {
    expect(normalizarTemaParcial({ color_primario: ' #abc ' })).toEqual({
      color_primario: '#ABC',
    });
  });

  it('valida el tema completo', () => {
    expect(() => validarTema({ ...TEMA_POR_DEFECTO })).not.toThrow();
    expect(() =>
      validarTema({ ...TEMA_POR_DEFECTO, color_primario: 'rojo' }),
    ).toThrow(DomainError);
    expect(() =>
      validarTema({ ...TEMA_POR_DEFECTO, radio_bordes: 33 }),
    ).toThrow(/radio/i);
    expect(() => validarTema({ ...TEMA_POR_DEFECTO, fuente: 'x' as never })).toThrow(
      /fuente/i,
    );
    expect(() => validarTema({ ...TEMA_POR_DEFECTO, modo: 'x' as never })).toThrow(/modo/i);
  });

  it('resuelve la herencia y el origen de cada capa', () => {
    const plataforma = {
      plantilla: 'minimal' as const,
      tema: { color_primario: '#111111' },
    };
    const empresa = {
      plantilla: 'boutique' as const,
      tema: { color_secundario: '#222222' },
    };

    const conTienda = resolverPersonalizacion(
      { plantilla: null, tema: { color_acento: '#333333' } },
      empresa,
      plataforma,
    );
    expect(conTienda.plantilla).toBe('boutique');
    expect(conTienda.tema).toMatchObject({
      color_primario: '#111111',
      color_secundario: '#222222',
      color_acento: '#333333',
    });
    expect(conTienda.origen).toEqual({ plantilla: 'empresa', tema: 'tienda' });

    const soloPlataforma = resolverPersonalizacion(
      { plantilla: null, tema: {} },
      { plantilla: null, tema: {} },
      plataforma,
    );
    expect(soloPlataforma.origen).toEqual({
      plantilla: 'plataforma',
      tema: 'plataforma',
    });

    const sinNada = resolverPersonalizacion(
      { plantilla: null, tema: {} },
      { plantilla: null, tema: {} },
      { plantilla: null, tema: {} },
    );
    expect(sinNada.plantilla).toBe('clasica');
    expect(sinNada.origen).toEqual({ plantilla: 'defecto', tema: 'defecto' });
  });

  it('genera CSS y resuelve helpers de enums', () => {
    expect(generarTemaCss({ ...TEMA_POR_DEFECTO, modo: 'auto' })).toContain(
      'color-scheme: light dark;',
    );
    expect(generarTemaCss({ ...TEMA_POR_DEFECTO, modo: 'oscuro' })).toContain(
      'color-scheme: dark;',
    );
    expect(plantillaDe('minimal')).toBe('minimal');
    expect(plantillaDe('x')).toBeNull();
    expect(esFuente('inter')).toBe(true);
    expect(esModo('sepia')).toBe(false);
    expect(esPlantilla('clasica')).toBe(true);
    expect(temaDe({ color_primario: '#abc' }).color_primario).toBe('#ABC');
  });
});
