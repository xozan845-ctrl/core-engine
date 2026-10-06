/**
 * Readiness agregada del gateway: comprueba que cada microservicio de
 * PRODUCCION responde en su `/health`. El gateway es stateless; este modulo
 * existe para que un deploy roto (servicios caidos) se detecte en el borde con
 * un `503` en `/ready`, en vez de devolver `200` "verde" con servicios `caido`.
 *
 * R-CD-12 (smoke): el smoke del entorno golpea `/ready`; si algun servicio no
 * esta `ok`, el deploy no debe darse por bueno.
 */

export interface ServicioSalud {
  nombre: string;
  puerto: number;
}

/**
 * Servicios que SI se despliegan en produccion (compose base). Fuera quedan
 * `commissions` (3006) y `market-intelligence` (3009): no estan en el compose
 * base (ADR-17), asi que no cuentan para readiness.
 */
export const SERVICIOS_PRODUCCION: ServicioSalud[] = [
  { nombre: 'identity', puerto: 3001 },
  { nombre: 'catalog', puerto: 3002 },
  { nombre: 'stores', puerto: 3003 },
  { nombre: 'orders', puerto: 3004 },
  { nombre: 'logistics', puerto: 3005 },
  { nombre: 'finance', puerto: 3007 },
  { nombre: 'field', puerto: 3008 },
];

export interface EstadoReadiness {
  status: 'ok' | 'degraded';
  servicios: Record<string, string>;
}

export type FetchComo = (
  url: string,
  init?: { signal?: AbortSignal },
) => Promise<{ ok: boolean; status: number }>;

/**
 * Consulta el `/health` de cada servicio. Devuelve `ok` solo si TODOS
 * responden `ok`; en caso contrario `degraded` con el detalle por servicio.
 */
export async function evaluarReadiness(
  servicios: ServicioSalud[] = SERVICIOS_PRODUCCION,
  fetchFn: FetchComo = fetch as unknown as FetchComo,
  timeoutMs = 1500,
): Promise<EstadoReadiness> {
  const estado: Record<string, string> = {};
  await Promise.all(
    servicios.map(async ({ nombre, puerto }) => {
      const url = `http://${nombre}-service:${puerto}/health`;
      try {
        const r = await fetchFn(url, { signal: AbortSignal.timeout(timeoutMs) });
        estado[nombre] = r.ok ? 'ok' : `error:${r.status}`;
      } catch {
        estado[nombre] = 'caido';
      }
    }),
  );
  const status: EstadoReadiness['status'] = Object.values(estado).every((v) => v === 'ok')
    ? 'ok'
    : 'degraded';
  return { status, servicios: estado };
}

/** Codigo HTTP del endpoint `/ready`: 200 si todo esta `ok`, 503 si degradado. */
export function codigoReadiness(estado: EstadoReadiness): number {
  return estado.status === 'ok' ? 200 : 503;
}
