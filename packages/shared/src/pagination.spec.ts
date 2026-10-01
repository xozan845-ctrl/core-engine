import { parsearPaginacion, crearPagina } from './pagination';

describe('pagination (paginacion consistente de la API)', () => {
  it('debe usar pagina=1 y limite=20 cuando la query no trae valores', () => {
    expect(parsearPaginacion({})).toEqual({ pagina: 1, limite: 20 });
    expect(parsearPaginacion({ pagina: undefined, limite: undefined })).toEqual({ pagina: 1, limite: 20 });
  });

  it('debe acotar pagina a minimo 1 cuando llega 0 o texto invalido', () => {
    expect(parsearPaginacion({ pagina: '0', limite: '5' })).toEqual({ pagina: 1, limite: 5 });
    expect(parsearPaginacion({ pagina: 'abc', limite: '5' })).toEqual({ pagina: 1, limite: 5 });
  });

  it('debe acotar limite a 100 cuando se excede el rango superior', () => {
    expect(parsearPaginacion({ pagina: '3', limite: '250' })).toEqual({ pagina: 3, limite: 100 });
  });

  it('debe usar el limite por defecto cuando el valor es 0 o texto invalido', () => {
    expect(parsearPaginacion({ pagina: '3', limite: '0' })).toEqual({ pagina: 3, limite: 20 });
    expect(parsearPaginacion({ pagina: '3', limite: 'abc' })).toEqual({ pagina: 3, limite: 20 });
  });

  it('debe crear la pagina con paginas = ceil(total / limite) cuando hay mas de una pagina', () => {
    const pagina = crearPagina(['a', 'b'], 5, { pagina: 1, limite: 2 });
    expect(pagina.items).toEqual(['a', 'b']);
    expect(pagina.total).toBe(5);
    expect(pagina.paginas).toBe(3);
  });

  it('debe devolver al menos una pagina cuando no hay registros', () => {
    const pagina = crearPagina([], 0, { pagina: 2, limite: 20 });
    expect(pagina.pagina).toBe(2);
    expect(pagina.total).toBe(0);
    expect(pagina.paginas).toBe(1);
  });
});