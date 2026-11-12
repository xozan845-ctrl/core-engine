import { DomainError } from '@core/shared';
import {
  PERSONALIZACION_POR_DEFECTO,
  TEMA_POR_DEFECTO,
  conSeoImagenUrl,
  generarTemaCss,
  normalizarPersonalizacion,
  normalizarTema,
  personalizacionDe,
  urlDestinoValida,
  validarPersonalizacion,
} from './personalizacion-tienda';

const IMAGEN = '11111111-2222-4333-8444-555555555555';

describe('personalización de tienda (ADR-39)', () => {
  describe('personalizacionDe', () => {
    it('devuelve los valores por defecto sin fila', () => {
      expect(personalizacionDe(null)).toEqual(PERSONALIZACION_POR_DEFECTO);
      expect(personalizacionDe(undefined).tema).toEqual(TEMA_POR_DEFECTO);
    });

    it('completa una fila parcial y resuelve el tema', () => {
      const fila = {
        plantilla: 'boutique' as const,
        tema: { color_primario: '#abc', fuente: 'poppins' },
        hero_titulo: '  Hola  ',
        seo_imagen_objeto_id: IMAGEN,
      };

      expect(personalizacionDe(fila)).toMatchObject({
        plantilla: 'boutique',
        tema: {
          color_primario: '#ABC',
          fuente: 'poppins',
          color_secundario: TEMA_POR_DEFECTO.color_secundario,
          radio_bordes: 12,
        },
        hero_titulo: 'Hola',
        seo_imagen_objeto_id: IMAGEN,
      });
    });

    it('cae a los valores por defecto si la fila trae datos inválidos', () => {
      const resultado = personalizacionDe({
        plantilla: 'inventada' as never,
        tema: { color_primario: 'rojo', fuente: 'comic' as never, modo: 'sepia' as never },
      });

      expect(resultado.plantilla).toBe('clasica');
      expect(resultado.tema.color_primario).toBe(TEMA_POR_DEFECTO.color_primario);
      expect(resultado.tema.fuente).toBe('system');
      expect(resultado.tema.modo).toBe('claro');
    });
  });

  describe('normalizarPersonalizacion', () => {
    it('mezcla solo los campos enviados y normaliza colores', () => {
      const actual = {
        ...personalizacionDe(null),
        plantilla: 'minimal' as const,
        tema: { ...TEMA_POR_DEFECTO, color_primario: '#111111' },
        hero_titulo: 'Antes',
      };

      const meta = normalizarPersonalizacion(
        { tema: { color_primario: ' #a1b2c3 ' }, hero_titulo: '  Después  ' },
        actual,
      );

      expect(meta.plantilla).toBe('minimal');
      expect(meta.tema.color_primario).toBe('#A1B2C3');
      expect(meta.tema.color_secundario).toBe(actual.tema.color_secundario);
      expect(meta.hero_titulo).toBe('Después');
    });

    it('limpia la imagen SEO con null explícito', () => {
      const actual = { ...personalizacionDe(null), seo_imagen_objeto_id: IMAGEN };
      expect(
        normalizarPersonalizacion({ seo_imagen_objeto_id: null }, actual).seo_imagen_objeto_id,
      ).toBeNull();
    });
  });

  describe('normalizarTema', () => {
    it('devuelve el tema actual sin datos y aplica defaults sin actual', () => {
      expect(normalizarTema(undefined, { ...TEMA_POR_DEFECTO, modo: 'oscuro' })).toEqual({
        ...TEMA_POR_DEFECTO,
        modo: 'oscuro',
      });
      expect(normalizarTema({})).toEqual(TEMA_POR_DEFECTO);
    });
  });

  describe('validarPersonalizacion', () => {
    const valido = () => personalizacionDe(null);

    it('acepta la personalización por defecto', () => {
      expect(() => validarPersonalizacion(valido())).not.toThrow();
    });

    it('rechaza colores que no son hexadecimales', () => {
      for (const color of ['red', '#12', '#12345', 'rgb(0,0,0)', '#GGGGGG']) {
        expect(() =>
          validarPersonalizacion({
            ...valido(),
            tema: { ...TEMA_POR_DEFECTO, color_primario: color },
          }),
        ).toThrow(DomainError);
      }
    });

    it('rechaza fuente, modo y radio inválidos', () => {
      expect(() =>
        validarPersonalizacion({
          ...valido(),
          tema: { ...TEMA_POR_DEFECTO, fuente: 'comic' as never },
        }),
      ).toThrow(/fuente/i);
      expect(() =>
        validarPersonalizacion({
          ...valido(),
          tema: { ...TEMA_POR_DEFECTO, modo: 'sepia' as never },
        }),
      ).toThrow(/modo/i);
      expect(() =>
        validarPersonalizacion({
          ...valido(),
          tema: { ...TEMA_POR_DEFECTO, radio_bordes: 33 },
        }),
      ).toThrow(/radio/i);
      expect(() =>
        validarPersonalizacion({
          ...valido(),
          tema: { ...TEMA_POR_DEFECTO, radio_bordes: 1.5 },
        }),
      ).toThrow(/radio/i);
    });

    it('rechaza plantilla inválida y textos demasiado largos', () => {
      expect(() =>
        validarPersonalizacion({ ...valido(), plantilla: 'otra' as never }),
      ).toThrow(/plantilla/i);
      expect(() =>
        validarPersonalizacion({ ...valido(), hero_titulo: 'x'.repeat(81) }),
      ).toThrow(/hero_titulo/i);
      expect(() =>
        validarPersonalizacion({ ...valido(), seo_descripcion: 'x'.repeat(301) }),
      ).toThrow(/seo_descripcion/i);
    });

    it('solo acepta rutas relativas o HTTPS en el CTA', () => {
      for (const url of ['/tienda', '/productos?x=1', 'https://ejemplo.com', '']) {
        expect(() =>
          validarPersonalizacion({ ...valido(), hero_cta_url: url }),
        ).not.toThrow();
      }
      for (const url of ['javascript:alert(1)', 'data:text/html,x', 'http://ejemplo.com', '//evil.com']) {
        expect(() =>
          validarPersonalizacion({ ...valido(), hero_cta_url: url }),
        ).toThrow(/hero_cta_url/i);
      }
    });

    it('rechaza una imagen SEO que no es UUID', () => {
      expect(() =>
        validarPersonalizacion({ ...valido(), seo_imagen_objeto_id: 'no-uuid' }),
      ).toThrow(/imagen SEO/i);
    });
  });

  describe('urlDestinoValida', () => {
    it('distingue rutas seguras de esquemas peligrosos', () => {
      expect(urlDestinoValida('')).toBe(true);
      expect(urlDestinoValida('/ruta')).toBe(true);
      expect(urlDestinoValida('https://ejemplo.com/x')).toBe(true);
      expect(urlDestinoValida('//evil.com')).toBe(false);
      expect(urlDestinoValida('http://ejemplo.com')).toBe(false);
      expect(urlDestinoValida('javascript:alert(1)')).toBe(false);
    });
  });

  describe('conSeoImagenUrl', () => {
    it('deriva la URL pública y la deja nula sin objeto', () => {
      expect(
        conSeoImagenUrl({ ...personalizacionDe(null), seo_imagen_objeto_id: IMAGEN })
          .seo_imagen_url,
      ).toBe(`/api/v2/storage/public/objetos/${IMAGEN}`);
      expect(conSeoImagenUrl(personalizacionDe(null)).seo_imagen_url).toBeNull();
    });
  });

  describe('generarTemaCss', () => {
    it('genera variables CSS desde el tema resuelto', () => {
      const css = generarTemaCss({
        ...TEMA_POR_DEFECTO,
        color_primario: '#112233',
        fuente: 'poppins',
        radio_bordes: 8,
        modo: 'oscuro',
      });

      expect(css).toContain('--tienda-color-primario: #112233;');
      expect(css).toContain('--tienda-fuente: Poppins, system-ui, sans-serif;');
      expect(css).toContain('--tienda-radio-bordes: 8px;');
      expect(css).toContain('--tienda-modo: oscuro;');
      expect(css).toContain('color-scheme: dark;');
    });

    it('usa light dark cuando el modo es auto', () => {
      expect(generarTemaCss({ ...TEMA_POR_DEFECTO, modo: 'auto' })).toContain(
        'color-scheme: light dark;',
      );
    });
  });
});
