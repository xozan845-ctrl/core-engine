import { DomainError } from './errors';
import {
  SLUG_MAX_LENGTH,
  esSlugValido,
  esUuid,
  normalizarSlug,
  slugConSufijo,
  slugDesdeNombre,
  validarSlugSolicitado,
} from './slug';

describe('slug (ADR-26)', () => {
  describe('normalizarSlug', () => {
    it('convierte nombres con acentos, mayúsculas y espacios', () => {
      expect(normalizarSlug('  Distribuidora Núñez & Cía.  ')).toBe('distribuidora-nunez-cia');
    });

    it('colapsa separadores y recorta guiones de los extremos', () => {
      expect(normalizarSlug('--Mi   tienda__de:ropa--')).toBe('mi-tienda-de-ropa');
    });

    it('recorta a la longitud máxima sin dejar un guion final', () => {
      const resultado = normalizarSlug(`${'a'.repeat(SLUG_MAX_LENGTH)} b`);
      expect(resultado).toHaveLength(SLUG_MAX_LENGTH);
      expect(resultado.endsWith('-')).toBe(false);
    });

    it('devuelve cadena vacía cuando no hay caracteres útiles', () => {
      expect(normalizarSlug('   ')).toBe('');
      expect(normalizarSlug('!!!')).toBe('');
    });

    it('neutraliza entradas maliciosas (R-U-9)', () => {
      expect(normalizarSlug("'; DROP TABLE empresas; --")).toBe('drop-table-empresas');
      expect(normalizarSlug('<script>alert(1)</script>')).toBe('script-alert-1-script');
      expect(normalizarSlug('{"slug":"x"}')).toBe('slug-x');
      expect(normalizarSlug('\u0000\u0001')).toBe('');
    });
  });

  describe('slugDesdeNombre', () => {
    it('deriva el slug del nombre', () => {
      expect(slugDesdeNombre('Tienda de la Vendedora Piloto')).toBe('tienda-de-la-vendedora-piloto');
    });

    it('usa el fallback cuando el nombre no aporta caracteres', () => {
      expect(slugDesdeNombre('!!!', 'mi-tienda')).toBe('mi-tienda');
    });

    it('completa nombres de un solo carácter', () => {
      expect(slugDesdeNombre('A')).toBe('a-1');
    });

    it('no deja que un nombre con formato UUID se confunda con una clave', () => {
      const uuid = '11111111-2222-4333-8444-555555555555';
      expect(esUuid(slugDesdeNombre(uuid))).toBe(false);
    });

    it('evita las palabras reservadas derivadas del nombre o del fallback', () => {
      expect(slugDesdeNombre('Admin')).toBe('admin-1');
      expect(slugDesdeNombre('!!!', 'empresa')).toBe('empresa-1');
      expect(esSlugValido(slugDesdeNombre('Tienda'))).toBe(true);
    });
  });

  describe('slugConSufijo', () => {
    it('añade el sufijo numérico', () => {
      expect(slugConSufijo('mi-tienda', 2)).toBe('mi-tienda-2');
    });

    it('recorta la base para no superar la longitud máxima', () => {
      const resultado = slugConSufijo('a'.repeat(SLUG_MAX_LENGTH), 12);
      expect(resultado).toHaveLength(SLUG_MAX_LENGTH);
      expect(resultado.endsWith('-12')).toBe(true);
    });

    it('usa un fallback si la base queda vacía', () => {
      expect(slugConSufijo('', 2)).toBe('entidad-2');
    });
  });

  describe('esSlugValido', () => {
    it('acepta slugs normalizados', () => {
      expect(esSlugValido('mi-tienda-2')).toBe(true);
    });

    it('rechaza vacíos, formatos inválidos, reservados y UUID', () => {
      expect(esSlugValido('')).toBe(false);
      expect(esSlugValido('-mi-tienda')).toBe(false);
      expect(esSlugValido('mi_tienda')).toBe(false);
      expect(esSlugValido('admin')).toBe(false);
      expect(esSlugValido('11111111-2222-4333-8444-555555555555')).toBe(false);
    });
  });

  describe('validarSlugSolicitado', () => {
    it('normaliza antes de validar', () => {
      expect(validarSlugSolicitado('  Mi Tienda  ')).toBe('mi-tienda');
    });

    it('rechaza un valor sin caracteres útiles', () => {
      expect(() => validarSlugSolicitado('!!!')).toThrow(DomainError);
    });

    it('rechaza palabras reservadas', () => {
      expect(() => validarSlugSolicitado('admin')).toThrow(/reservado/);
    });

    it('rechaza un slug con formato de UUID', () => {
      expect(() => validarSlugSolicitado('11111111-2222-4333-8444-555555555555')).toThrow(
        /UUID/,
      );
    });
  });

  describe('esUuid', () => {
    it('distingue un UUID de un slug', () => {
      expect(esUuid('11111111-2222-4333-8444-555555555555')).toBe(true);
      expect(esUuid('mi-tienda')).toBe(false);
    });
  });
});
