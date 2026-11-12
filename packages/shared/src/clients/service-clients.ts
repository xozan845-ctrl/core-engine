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

function financeUrl(): string {
  return (process.env.FINANCE_SERVICE_URL ?? 'http://finance-service:3007').replace(/\/+$/, '');
}

export interface CreditoOrdenResumen {
  credito_id: string;
  balance_cents: number;
  limite_cents: number;
}

export type ResultadoCreditoOrden =
  | { estado: 'ok'; credito: CreditoOrdenResumen }
  | { estado: 'rechazado' }
  | { estado: 'error' };

/** Registra la cuenta por cobrar de una orden a crédito (idempotente por order_id). */
export async function registrarCreditoOrden(input: {
  empresa_id: string;
  parte_id: string;
  order_id: string;
  monto_cents: number;
  concepto: string;
  vendedor_id?: string;
}): Promise<ResultadoCreditoOrden> {
  const res = await pedir<CreditoOrdenResumen>(`${financeUrl()}/internal/finance/creditos`, {
    method: 'POST',
    body: JSON.stringify(input),
  });
  if (res.status === 409) return { estado: 'rechazado' };
  if (res.status === 0 || !res.data) return { estado: 'error' };
  return { estado: 'ok', credito: res.data };
}

/** Compensa el crédito si la orden no llega a crearse. */
export async function anularCreditoOrden(orderId: string): Promise<boolean> {
  const res = await pedir<{ ok: boolean }>(
    `${financeUrl()}/internal/finance/creditos/orden/${encodeURIComponent(orderId)}`,
    { method: 'DELETE' },
  );
  return res.status >= 200 && res.status < 300 && !!res.data;
}

function catalogUrl(): string {
  return (process.env.CATALOG_SERVICE_URL ?? 'http://catalog-service:3002').replace(/\/+$/, '');
}

function schedulingUrl(): string {
  return (process.env.SCHEDULING_SERVICE_URL ?? 'http://scheduling-service:3011').replace(/\/+$/, '');
}

export interface CotizacionExterna {
  total_cents: number;
  dias: number;
}

/** Cotiza un item por rango de fechas (tarifas + precio base). */
export async function cotizarItemReserva(input: {
  empresa_id: string;
  item_id: string;
  desde: string;
  hasta: string;
}): Promise<CotizacionExterna | null> {
  const query = new URLSearchParams({
    empresa_id: input.empresa_id,
    desde: input.desde,
    hasta: input.hasta,
  });
  const res = await pedir<CotizacionExterna>(
    `${catalogUrl()}/internal/items/${encodeURIComponent(input.item_id)}/cotizacion?${query.toString()}`,
  );
  if (res.status === 0 || !res.data) return null;
  return res.data;
}

/** `true`/`false` si scheduling respondió; `null` si no está disponible. */
export async function verificarDisponibilidadReserva(input: {
  empresa_id: string;
  recurso_id: string;
  inicia_en: string;
  termina_en: string;
}): Promise<boolean | null> {
  const res = await pedir<{ disponible: boolean }>(
    `${schedulingUrl()}/internal/scheduling/disponibilidad`,
    { method: 'POST', body: JSON.stringify(input) },
  );
  if (res.status === 409) return false;
  if (res.status === 0 || !res.data) return null;
  return res.data.disponible === true;
}
