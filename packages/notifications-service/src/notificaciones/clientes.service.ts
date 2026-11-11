import { Injectable } from '@nestjs/common';
import { esUuid, timeoutInternoMs } from '@core/shared';
import type { OrdenExterna, UsuarioExterno } from './tipos';

/**
 * Resolución de datos por las APIs internas (ADR-0002/ADR-0003):
 * - orders: detalle y comprador/vendedor de la orden.
 * - identity: nombre y correo de cada usuario.
 *
 * No se leen esquemas ajenos. Un 404 se trata como "no disponible" (no se
 * reintenta); un fallo de red o 5xx se propaga para que Rabbit reintente/DLQ.
 */
@Injectable()
export class ClientesService {
  private readonly ordersUrl = (process.env.ORDERS_SERVICE_URL ?? 'http://orders-service:3004').replace(/\/+$/, '');
  private readonly identityUrl = (process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001').replace(/\/+$/, '');
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  async obtenerOrden(orderId: string): Promise<OrdenExterna | null> {
    if (!esUuid(orderId)) return null;
    return this.pedir<OrdenExterna>(`${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}`, 'orders');
  }

  async obtenerUsuario(usuarioId: string): Promise<UsuarioExterno | null> {
    if (!esUuid(usuarioId)) return null;
    return this.pedir<UsuarioExterno>(`${this.identityUrl}/internal/usuarios/${encodeURIComponent(usuarioId)}`, 'identity');
  }

  private async pedir<T>(url: string, servicio: string): Promise<T | null> {
    let respuesta: Response;
    try {
      respuesta = await fetch(url, {
        headers: { 'x-internal-key': this.claveInterna },
        // R-RB-4: fallar rápido en vez de esperar el timeout del proxy.
        signal: AbortSignal.timeout(timeoutInternoMs()),
      });
    } catch {
      throw new Error(`No se pudo consultar ${servicio}.`);
    }
    if (respuesta.status === 404) return null;
    if (!respuesta.ok) {
      throw new Error(`${servicio} respondió ${respuesta.status}.`);
    }
    return (await respuesta.json()) as T;
  }
}
