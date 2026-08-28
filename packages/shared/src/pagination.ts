/**
 * Paginacion consistente de la API (doc: paginacion consistente en todas las respuestas).
 */
export interface PaginacionParams {
  pagina: number;
  limite: number;
}

export interface Pagina<T> {
  items: T[];
  total: number;
  pagina: number;
  limite: number;
  paginas: number;
}

export function parsearPaginacion(query: Record<string, unknown>): PaginacionParams {
  const pagina = Math.max(1, parseInt(String(query.pagina ?? '1'), 10) || 1);
  const limite = Math.min(100, Math.max(1, parseInt(String(query.limite ?? '20'), 10) || 20));
  return { pagina, limite };
}

export function crearPagina<T>(
  items: T[],
  total: number,
  { pagina, limite }: PaginacionParams,
): Pagina<T> {
  return {
    items,
    total,
    pagina,
    limite,
    paginas: Math.max(1, Math.ceil(total / limite)),
  };
}