/**
 * Clientes internos (servicio→servicio) para no leer/escribir esquemas ajenos.
 * Cada llamada usa `x-internal-key` y un timeout corto; el llamador decide si
 * un fallo se traduce a 503 (fail-closed) o a "no encontrado".
 */

export interface CapacidadExterna {
  empresa_id: string;
  capacidad_codigo: string;
  activo: boolean;
}

export interface TiendaExterna {
  id: string;
  nombre: string;
  empresa_id: string;
  vendedor_id: string | null;
  participacion_porcentaje?: number | null;
}

export type ConsultaTienda =
  | { estado: 'ok'; tienda: TiendaExterna }
  | { estado: 'no_encontrada' }
  | { estado: 'error' };

export type ResultadoAsignacion = 'ok' | 'conflicto' | 'error';

interface Respuesta<T> {
  status: number;
  data?: T;
}

function identityUrl(): string {
  return (process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001').replace(/\/+$/, '');
}

function storesUrl(): string {
  return (process.env.STORES_SERVICE_URL ?? 'http://stores-service:3003').replace(/\/+$/, '');
}

/** Timeout de las llamadas internas; reutilizado por los `fetch` de los servicios. */
export function timeoutInternoMs(): number {
  const valor = Number(process.env.INTERNAL_REQUEST_TIMEOUT_MS ?? 3000);
  return Number.isFinite(valor) && valor > 0 ? valor : 3000;
}

async function pedir<T>(url: string, init?: RequestInit): Promise<Respuesta<T>> {
  try {
    const res = await fetch(url, {
      ...init,
      headers: {
        'content-type': 'application/json',
        'x-internal-key': process.env.INTERNAL_API_KEY ?? '',
        ...(init?.headers ?? {}),
      },
      signal: AbortSignal.timeout(timeoutInternoMs()),
    });
    if (!res.ok) return { status: res.status };
    return { status: res.status, data: (await res.json()) as T };
  } catch {
    return { status: 0 };
  }
}

/** Capacidades activas por empresa (lote). `null` = identity no disponible. */
export async function capacidadesDeEmpresas(
  empresaIds: string[],
  capacidades: string[],
): Promise<CapacidadExterna[] | null> {
  if (empresaIds.length === 0 || capacidades.length === 0) return [];
  const res = await pedir<{ capacidades: CapacidadExterna[] }>(
    `${identityUrl()}/internal/empresas/capacidades`,
    {
      method: 'POST',
      body: JSON.stringify({ empresa_ids: empresaIds, capacidades }),
    },
  );
  if (res.status === 0 || !res.data) return null;
  return res.data.capacidades;
}

/** `true`/`false` si identity respondió; `null` si no está disponible. */
export async function partePerteneceAEmpresa(
  empresaId: string,
  parteId: string,
): Promise<boolean | null> {
  const res = await pedir<{ id: string }>(
    `${identityUrl()}/internal/partes/${encodeURIComponent(parteId)}?empresa_id=${encodeURIComponent(empresaId)}`,
  );
  if (res.status === 404) return false;
  if (res.status === 0 || !res.data) return null;
  return true;
}

export async function obtenerTienda(tiendaId: string): Promise<ConsultaTienda> {
  const res = await pedir<TiendaExterna>(
    `${storesUrl()}/internal/tiendas/${encodeURIComponent(tiendaId)}`,
  );
  if (res.status === 404) return { estado: 'no_encontrada' };
  if (res.status === 0 || !res.data) return { estado: 'error' };
  return { estado: 'ok', tienda: res.data };
}

/** Tiendas por lote. `null` = stores no disponible. */
export async function tiendasPorIds(ids: string[]): Promise<TiendaExterna[] | null> {
  if (ids.length === 0) return [];
  const res = await pedir<{ tiendas: TiendaExterna[] }>(
    `${storesUrl()}/internal/tiendas?ids=${encodeURIComponent(ids.join(','))}`,
  );
  if (res.status === 0 || !res.data) return null;
  return res.data.tiendas;
}

/** Tiendas de una empresa. `null` = stores no disponible. */
export async function tiendasPorEmpresa(empresaId: string): Promise<TiendaExterna[] | null> {
  const res = await pedir<{ tiendas: TiendaExterna[] }>(
    `${storesUrl()}/internal/tiendas/empresa/${encodeURIComponent(empresaId)}`,
  );
  if (res.status === 0 || !res.data) return null;
  return res.data.tiendas;
}

/** Tienda asignada a un vendedor. */
export async function tiendaDeVendedor(vendedorId: string): Promise<ConsultaTienda> {
  const res = await pedir<{ tienda: TiendaExterna | null }>(
    `${storesUrl()}/internal/tiendas/vendedor/${encodeURIComponent(vendedorId)}`,
  );
  if (res.status === 404) return { estado: 'no_encontrada' };
  if (res.status === 0 || !res.data) return { estado: 'error' };
  if (!res.data.tienda) return { estado: 'no_encontrada' };
  return { estado: 'ok', tienda: res.data.tienda };
}

export async function asignarVendedorATienda(
  tiendaId: string,
  usuarioId: string,
  participacion: number,
): Promise<ResultadoAsignacion> {
  const res = await pedir<{ ok: true }>(
    `${storesUrl()}/internal/tiendas/${encodeURIComponent(tiendaId)}/asignar-vendedor`,
    {
      method: 'POST',
      body: JSON.stringify({ usuario_id: usuarioId, participacion }),
    },
  );
  if (res.status === 409) return 'conflicto';
  if (res.status === 0 || !res.data) return 'error';
  return 'ok';
}

export async function desasignarVendedorDeTienda(
  tiendaId: string,
  usuarioId: string,
): Promise<boolean> {
  const res = await pedir<{ ok: true }>(
    `${storesUrl()}/internal/tiendas/${encodeURIComponent(tiendaId)}/desasignar-vendedor`,
    {
      method: 'POST',
      body: JSON.stringify({ usuario_id: usuarioId }),
    },
  );
  return res.status !== 0 && !!res.data;
}
