import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import { PgService, DomainError, ForbiddenError, Logger, Money, NotFoundError } from '@core/shared';

export interface Envio {
  id: string;
  guia: string;
  order_id: string;
  empresa_id: string;
  modalidad_logistica: ModalidadLogistica;
  proveedor_tipo: ProveedorTipo | null;
  proveedor_tenant_id: string | null;
  estado: string;
  creado_en: string;
}

export interface OrdenExterna {
  id: string;
  cliente_id: string;
  items: { sku: string; cantidad: number; producto_nombre: string; empresa_id: string; precio_unitario_cents: number }[];
  total: string;
  total_cents: number;
  estado: string;
}

export type ModalidadLogistica = 'plataforma' | 'propia' | 'hibrida';
export type ProveedorTipo = 'plataforma' | 'empresa';
interface EmpresaExterna { id: string; modalidad_logistica: ModalidadLogistica }

/**
 * Logistica: el sistema logístico automatiza la operacion de la bodega
 * (Tabla 1). Genera la guia de despacho y notifica a las partes.
 */
@Injectable()
export class EnviosService {
  private readonly ordersUrl = process.env.ORDERS_SERVICE_URL ?? 'http://orders-service:3004';
  private readonly identityUrl = process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly tenantPlataforma = process.env.PLATFORM_LOGISTICS_TENANT_ID ?? '00000000-0000-0000-0000-000000000001';
  private readonly logger = Logger.create('envios');

  constructor(private readonly pg: PgService) {}

  /** Prepara la guia de despacho cuando el pago fue procesado (payment.procesado). */
  async prepararEnvio(
    orderId: string,
    empresaId: string,
    montoCents: number,
    modalidad: ModalidadLogistica,
  ): Promise<Envio> {
    const existente = await this.pg.queryOne<Envio>(
      `SELECT * FROM logistics.envios WHERE order_id = $1 AND empresa_id = $2`,
      [orderId, empresaId],
    );
    if (existente) {
      if (existente.proveedor_tipo && existente.proveedor_tenant_id) {
        await this.registrarAsignacion(
          existente.id, null, existente.proveedor_tipo,
          existente.proveedor_tenant_id, 'automatica',
        );
      }
      return existente;
    }

    const guia = `BGH-${Date.now().toString(36).toUpperCase()}-${randomUUID().slice(0, 6).toUpperCase()}`;
    const proveedor = this.proveedorAutomatico(modalidad, empresaId);
    const fila = await this.pg.queryOne<Envio>(
      `INSERT INTO logistics.envios
        (id, guia, order_id, empresa_id, modalidad_logistica, proveedor_tipo,
         proveedor_tenant_id, monto_cents, estado)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'en_preparacion') RETURNING *`,
      [randomUUID(), guia, orderId, empresaId, modalidad, proveedor.tipo, proveedor.tenantId, montoCents],
    );
    if (!fila) throw new DomainError('ENVIO_FALLIDO', 'No se pudo preparar el envio.');
    if (proveedor.tipo && proveedor.tenantId) {
      await this.registrarAsignacion(fila.id, null, proveedor.tipo, proveedor.tenantId, 'automatica');
    }
    this.logger.info({ msg: 'Guia de despacho generada', order_id: orderId, guia: fila.guia });
    return fila;
  }

  async prepararEnviosDeOrden(orderId: string): Promise<Envio[]> {
    const orden = await this.consultarOrden(orderId);
    if (!orden?.items.length) throw new DomainError('ORDEN_SIN_LINEAS', 'La orden no tiene líneas para despachar.');
    const montos = new Map<string, number>();
    for (const item of orden.items) {
      if (!item.empresa_id) throw new DomainError('EMPRESA_REQUERIDA', 'Una línea no tiene empresa asignada.');
      const subtotal = item.precio_unitario_cents * item.cantidad;
      montos.set(item.empresa_id, (montos.get(item.empresa_id) ?? 0) + subtotal);
    }
    const envios: Envio[] = [];
    for (const [empresaId, monto] of montos) {
      const empresa = await this.consultarEmpresa(empresaId);
      envios.push(await this.prepararEnvio(orderId, empresaId, monto, empresa.modalidad_logistica));
    }
    return envios;
  }

  private proveedorAutomatico(modalidad: ModalidadLogistica, empresaId: string): { tipo: ProveedorTipo | null; tenantId: string | null } {
    if (modalidad === 'plataforma') return { tipo: 'plataforma', tenantId: this.tenantPlataforma };
    if (modalidad === 'propia') return { tipo: 'empresa', tenantId: empresaId };
    return { tipo: null, tenantId: null };
  }

  async asignarProveedor(
    envioId: string,
    empresaId: string,
    tipo: ProveedorTipo,
    actorUsuarioId?: string,
  ): Promise<Envio> {
    const envio = await this.pg.queryOne<Envio>(
      `SELECT * FROM logistics.envios WHERE id = $1 AND empresa_id = $2`, [envioId, empresaId],
    );
    if (!envio) throw new DomainError('ENVIO_NO_ENCONTRADO', 'El envío no pertenece a la empresa activa.');
    if (envio.modalidad_logistica !== 'hibrida') {
      throw new DomainError('ASIGNACION_FIJA', 'Solo un envío híbrido permite elegir proveedor.');
    }
    const tenant = tipo === 'plataforma' ? this.tenantPlataforma : empresaId;
    const actualizado = await this.pg.queryOne<Envio>(
      `UPDATE logistics.envios SET proveedor_tipo = $1, proveedor_tenant_id = $2
       WHERE id = $3 AND empresa_id = $4 RETURNING *`,
      [tipo, tenant, envioId, empresaId],
    );
    if (!actualizado) throw new DomainError('ENVIO_NO_ENCONTRADO', 'No se pudo asignar el envío.');
    await this.registrarAsignacion(
      envioId, envio.proveedor_tipo, tipo, tenant, 'empresario', actorUsuarioId,
    );
    return actualizado;
  }

  private async registrarAsignacion(
    envioId: string,
    anterior: ProveedorTipo | null,
    nuevo: ProveedorTipo,
    proveedorTenantId: string,
    origen: 'automatica' | 'empresario',
    actorUsuarioId?: string,
  ): Promise<void> {
    await this.pg.query(
      `INSERT INTO logistics.asignaciones_envio
        (envio_id, proveedor_tipo_anterior, proveedor_tipo_nuevo,
         proveedor_tenant_id, origen, actor_usuario_id)
       VALUES ($1,$2,$3,$4,$5,$6)
       ${origen === 'automatica' ? 'ON CONFLICT DO NOTHING' : ''}`,
      [envioId, anterior, nuevo, proveedorTenantId, origen, actorUsuarioId ?? null],
    );
  }

  async deEmpresa(empresaId: string): Promise<Envio[]> {
    return this.pg.query<Envio>(
      `SELECT * FROM logistics.envios WHERE empresa_id = $1 ORDER BY creado_en DESC LIMIT 200`,
      [empresaId],
    );
  }

  async avanzarEstadoEnvio(
    envioId: string,
    proveedorTenantId: string,
    estado: 'enviada' | 'entregada' | 'cancelada',
    motivo?: string,
  ): Promise<Envio> {
    const envio = await this.pg.queryOne<Envio>(
      `UPDATE logistics.envios SET estado = $1
       WHERE id = $2 AND proveedor_tenant_id = $3
       RETURNING *`,
      [estado, envioId, proveedorTenantId],
    );
    if (!envio) throw new DomainError('ENVIO_NO_ENCONTRADO', 'El envío no pertenece a la flota activa.');

    const estadosAceptados = estado === 'enviada' ? ['enviada', 'entregada'] : [estado];
    const pendientes = await this.pg.queryOne<{ total: number }>(
      `SELECT COUNT(*)::int AS total FROM logistics.envios
       WHERE order_id = $1 AND NOT (estado = ANY($2::text[]))`,
      [envio.order_id, estadosAceptados],
    );
    if ((pendientes?.total ?? 1) === 0) {
      await this.avanzarEstadoOrden(envio.order_id, estado, motivo);
    }
    return envio;
  }

  async deOrden(orderId: string): Promise<Envio | null> {
    return this.pg.queryOne<Envio>(
      `SELECT * FROM logistics.envios WHERE order_id = $1 ORDER BY creado_en LIMIT 1`,
      [orderId],
    );
  }

  private async consultarEmpresa(empresaId: string): Promise<EmpresaExterna> {
    try {
      const res = await fetch(`${this.identityUrl}/internal/empresas/${encodeURIComponent(empresaId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) throw new Error('empresa no disponible');
      return (await res.json()) as EmpresaExterna;
    } catch {
      throw new DomainError('EMPRESA_NO_DISPONIBLE', 'No se pudo resolver la modalidad logística de la empresa.');
    }
  }

  async listar(): Promise<(Envio & { monto: string })[]> {
    const filas = await this.pg.query<Envio & { monto_cents: number }>(
      `SELECT * FROM logistics.envios
       ORDER BY creado_en DESC LIMIT 200`,
    );
    return filas.map((f) => ({ ...f, monto: Money.desdeCentavos(f.monto_cents).string() }));
  }

  /** PATCH /orders/:id/estado: avanza el ciclo de vida (Tabla 13). */
  async avanzarEstadoOrden(
    orderId: string,
    estadoNuevo: string,
    motivo?: string,
    empresaId?: string,
  ): Promise<OrdenExterna> {
    if (empresaId) {
      const orden = await this.consultarOrden(orderId);
      if (!orden) throw new NotFoundError('Orden', orderId);
      if (!orden.items.some((item) => item.empresa_id === empresaId)) {
        throw new ForbiddenError('La orden no pertenece a la empresa activa.');
      }
    }
    const url = `${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}/transicion?estado=${encodeURIComponent(estadoNuevo)}${motivo ? `&motivo=${encodeURIComponent(motivo)}` : ''}`;
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'x-internal-key': this.claveInterna },
    });
    if (!res.ok) {
      const cuerpo = (await res.json().catch(() => ({}))) as { codigo?: string; mensaje?: string };
      throw new DomainError(
        cuerpo.codigo ?? 'TRANSICION_FALLIDA',
        cuerpo.mensaje ?? 'La transicion de estado fue rechazada.',
      );
    }
    const orden = (await res.json()) as OrdenExterna;
    this.logger.info({ msg: 'Estado de orden avanzado', order_id: orderId, estado: estadoNuevo });
    return orden;
  }

  /** Consulta una orden (para notificaciones y guias). */
  async consultarOrden(orderId: string): Promise<OrdenExterna | null> {
    try {
      const res = await fetch(`${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return null;
      return (await res.json()) as OrdenExterna;
    } catch {
      return null;
    }
  }
}
