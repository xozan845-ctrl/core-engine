import { DomainError } from '@core/shared';
import {
  CONTENIDO_POR_DEFECTO,
  POLITICAS_POR_DEFECTO,
  contenidoDe,
  normalizarContenido,
  validarContenido,
} from './contenido-tienda';

describe('contenido del storefront de tienda (ADR-40)', () => {
  describe('contenidoDe', () => {
    it('devuelve los valores por defecto sin fila', () => {
      expect(contenidoDe(null)).toEqual(CONTENIDO_POR_DEFECTO);
    });

    it('completa una fila parcial y ordena las secciones', () => {
      const contenido = contenidoDe({
        secciones: [
          { tipo: 'ofertas', visible: true, orden: 2 },
          { tipo: 'hero', visible: false, orden: 0 },
        ],
        sobre_nosotros: '  Hola  ',
        politicas: { envio: '24h' },
        redes: [{ red: 'instagram', url: 'https://instagram.com/x' }],
        anuncio_activo: true,
      });

      expect(contenido.secciones.map((s) => s.tipo)).toEqual(['hero', 'ofertas']);
      expect(contenido.sobre_nosotros).toBe('Hola');
      expect(contenido.politicas).toEqual({
        envio: '24h',
        devolucion: '',
        privacidad: '',
      });
      expect(contenido.redes).toHaveLength(1);
      expect(contenido.anuncio_activo).toBe(true);
    });

    it('acepta JSON serializado y descarta elementos inválidos al leer', () => {
      const contenido = contenidoDe({
        secciones: JSON.stringify([
          { tipo: 'hero', visible: true, orden: 0 },
          { tipo: 'inventada', visible: true, orden: 1 },
          { tipo: 'ofertas', visible: true, orden: 2 },
        ]),
        redes: JSON.stringify([
          { red: 'instagram', url: 'https://instagram.com/x' },
          { red: 'inventada', url: 'https://x.com' },
          { red: 'facebook', url: '' },
        ]),
      });

      expect(contenido.secciones.map((s) => s.tipo)).toEqual(['hero', 'ofertas']);
      expect(contenido.redes).toEqual([
        { red: 'instagram', url: 'https://instagram.com/x' },
      ]);
    });
  });

  describe('normalizarContenido', () => {
    it('mezcla solo los campos enviados y ordena las secciones', () => {
      const actual = {
        ...contenidoDe(null),
        sobre_nosotros: 'Antes',
        politicas: { ...POLITICAS_POR_DEFECTO, envio: '24h' },
      };

      const meta = normalizarContenido(
        {
          secciones: [
            { tipo: 'ofertas', visible: true, orden: 1 },
            { tipo: 'hero', visible: true, orden: 0 },
          ],
          sobre_nosotros: '  Después  ',
          politicas: { devolucion: '7 días' },
        },
        actual,
      );

      expect(meta.secciones.map((s) => s.tipo)).toEqual(['hero', 'ofertas']);
      expect(meta.sobre_nosotros).toBe('Después');
      expect(meta.politicas).toEqual({
        envio: '24h',
        devolucion: '7 días',
        privacidad: '',
      });
    });

    it('limpia secciones y redes con null o array vacío', () => {
      const actual = {
        ...contenidoDe(null),
        secciones: [{ tipo: 'hero' as const, visible: true, orden: 0 }],
        redes: [{ red: 'instagram' as const, url: 'https://instagram.com/x' }],
      };

      expect(normalizarContenido({ secciones: null, redes: null }, actual)).toMatchObject({
        secciones: [],
        redes: [],
      });
      expect(normalizarContenido({ secciones: [], redes: [] }, actual)).toMatchObject({
        secciones: [],
        redes: [],
      });
    });
  });

  describe('validarContenido', () => {
    const valido = () => contenidoDe(null);

    it('acepta el contenido por defecto', () => {
      expect(() => validarContenido(valido())).not.toThrow();
    });

    it('rechaza secciones repetidas, tipos inválidos u órdenes inválidos', () => {
      expect(() =>
        validarContenido({
          ...valido(),
          secciones: [
            { tipo: 'hero', visible: true, orden: 0 },
            { tipo: 'hero', visible: true, orden: 1 },
          ],
        }),
      ).toThrow(/repetida/i);

      expect(() =>
        validarContenido({
          ...valido(),
          secciones: [
            { tipo: 'hero', visible: true, orden: 0 },
            { tipo: 'ofertas', visible: true, orden: 0 },
          ],
        }),
      ).toThrow(/orden 0/i);

      expect(() =>
        validarContenido({
          ...valido(),
          secciones: [{ tipo: 'inventada' as never, visible: true, orden: 0 }],
        }),
      ).toThrow(/no es válida/i);

      expect(() =>
        validarContenido({
          ...valido(),
          secciones: [{ tipo: 'hero', visible: true, orden: 1.5 }],
        }),
      ).toThrow(/entero/i);
    });

    it('rechaza más secciones o redes de las permitidas', () => {
      const secciones = Array.from({ length: 13 }, (_, i) => ({
        tipo: 'hero' as const,
        visible: true,
        orden: i,
      }));
      expect(() => validarContenido({ ...valido(), secciones })).toThrow(/más de 12/i);

      const redes = Array.from({ length: 7 }, (_, i) => ({
        red: 'instagram' as const,
        url: `https://instagram.com/${i}`,
      }));
      expect(() => validarContenido({ ...valido(), redes })).toThrow(/más de 6/i);
    });

    it('rechaza textos demasiado largos', () => {
      expect(() =>
        validarContenido({ ...valido(), sobre_nosotros: 'x'.repeat(2001) }),
      ).toThrow(/sobre_nosotros/i);
      expect(() =>
        validarContenido({
          ...valido(),
          politicas: { ...POLITICAS_POR_DEFECTO, envio: 'x'.repeat(1001) },
        }),
      ).toThrow(/envío|envio/i);
      expect(() =>
        validarContenido({ ...valido(), anuncio_texto: 'x'.repeat(121) }),
      ).toThrow(/anuncio/i);
    });

    it('exige HTTPS en redes y permite ruta relativa o HTTPS en el anuncio', () => {
      for (const url of ['http://instagram.com/x', 'javascript:alert(1)', '//x.com']) {
        expect(() =>
          validarContenido({
            ...valido(),
            redes: [{ red: 'instagram', url }],
          })
        ).toThrow(/HTTPS/i);
      }

      for (const url of ['', '/productos', 'https://ejemplo.com']) {
        expect(() =>
          validarContenido({ ...valido(), anuncio_enlace: url }),
        ).not.toThrow();
      }
      expect(() =>
        validarContenido({ ...valido(), anuncio_enlace: 'javascript:alert(1)' }),
      ).toThrow(DomainError);
    });
  });
});
