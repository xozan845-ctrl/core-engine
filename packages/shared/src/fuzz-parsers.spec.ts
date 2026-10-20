import fc from 'fast-check';
import { crearPagina, parsearPaginacion } from './pagination';
import { parametrizarRuta } from './metrics/metrics.service';

describe('fuzz de parsers (R-RB-1..4)', () => {
  it('parsearPaginacion nunca lanza y acota pagina/limite', () => {
    fc.assert(
      fc.property(fc.dictionary(fc.string(), fc.anything()), (query) => {
        const { pagina, limite } = parsearPaginacion(query);
        expect(Number.isInteger(pagina)).toBe(true);
        expect(pagina).toBeGreaterThanOrEqual(1);
        expect(Number.isInteger(limite)).toBe(true);
        expect(limite).toBeGreaterThanOrEqual(1);
        expect(limite).toBeLessThanOrEqual(100);
      }),
    );
  });

  it('crearPagina calcula al menos una pagina y conserva los items', () => {
    fc.assert(
      fc.property(
        fc.array(fc.anything()),
        fc.nat({ max: 1_000_000 }),
        fc.integer({ min: 1, max: 100 }),
        (items, total, limite) => {
          const pagina = crearPagina(items, total, { pagina: 1, limite });
          expect(pagina.items).toBe(items);
          expect(pagina.paginas).toBeGreaterThanOrEqual(1);
          expect(pagina.paginas).toBe(Math.max(1, Math.ceil(total / limite)));
        },
      ),
    );
  });

  it('parametrizarRuta es idempotente y no lanza con entradas arbitrarias', () => {
    fc.assert(
      fc.property(fc.string(), (ruta) => {
        const unaVez = parametrizarRuta(ruta);
        expect(unaVez.startsWith('/')).toBe(true);
        expect(unaVez).not.toContain('//');
        expect(unaVez).not.toMatch(/[?#]/);
        expect(parametrizarRuta(unaVez)).toBe(unaVez);
      }),
    );
  });
});
