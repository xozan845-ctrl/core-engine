import { Injectable } from '@nestjs/common';
import { PgService, OutboxService, Money, DomainError, NotFoundError, EVENTOS, Logger } from '@core/shared';
import { ContabilidadService } from '../contabilidad/contabilidad.service';

export interface Comprobante {
  id: string;
  serie: string;
  numero: string;
  tipo: 'FACTURA' | 'NOTA_CREDITO';
  orden_id?: string;
  cliente_id?: string;
  base_gravada_cents: number;
  iva_cents: number;
  exento_cents: number;
  total_cents: number;
  moneda: string;
  estado: 'BORRADOR' | 'EMITIDO' | 'ANULADO';
  datos_cliente?: unknown;
  emitido_en?: string;
  creado_en: string;
}

export interface SerieComprobante {
  serie: string;
  tipo: 'FACTURA' | 'NOTA_CREDITO';
  prefijo: string;
  secuencial_actual: number;
  ruc_emisor: string;
  jurisdiccion: string;
  activa: boolean;
}

/**
 * Comprobantes fiscales DGI (cap. 4.4): series con prefijo, secuenciales
 * consecutivos y RUC del emisor. Ley 842: facturacion y liquidacion en
 * cordobas (C$). Ley 822: IVA del 15 % se traslada al comprador.
 *
 * MVP: pagos simulados, por lo que los comprobantes nacen como BORRADOR
 * (EMITIR_COMPROBANTES_FISCALES=false). Al activar la facturacion real,
 * la emision consume el secuencial y publica `comprobante.emitido`.
 */
@Injectable()
export class FacturacionService {
  private readonly ordersUrl = process.env.ORDERS_SERVICE_URL ?? 'http://orders-service:3004';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly emitirComprobantes = (process.env.EMITIR_COMPROBANTES_FISCALES ?? 'false') === 'true';
  private readonly logger = Logger.create('finance.facturacion');

  constructor(
    private readonly pg: PgService,
    private readonly outbox: OutboxService,
    private readonly contabilidad: ContabilidadService,
  ) {}

  /** GET /api/v1/finanzas/comprobantes — listado con filtros. */
  async listar(desde?: string, hasta?: string, estado?: string): Promise<Comprobante[]> {
    return this.pg.query<Comprobante>(
      `SELECT id, serie, numero, tipo, orden_id, cliente_id, base_gravada_cents, iva_cents,
              exento_cents, total_cents, moneda, estado, datos_cliente, emitido_en, creado_en
       FROM finance.comprobantes
       WHERE ($1::timestamptz IS NULL OR creado_en >= $1)
         AND ($2::timestamptz IS NULL OR creado_en <= $2)
         AND ($3::text IS NULL OR estado = $3)
       ORDER BY creado_en DESC
       LIMIT 300`,
      [desde ?? null, hasta ?? null, estado ?? null],
    );
  }

  /**
   * Emite (o reserva como BORRADOR en el MVP) un comprobante de la venta.
   * base_gravada = total de la orden; IVA = base x tasa de la jurisdiccion.
   */
  async emitir(
    tipo: 'FACTURA' | 'NOTA_CREDITO',
    ordenId: string,
    datosCliente?: unknown,
  ): Promise<Comprobante> {
    const orden = await this.consultarOrden(ordenId);
    if (!orden) throw new NotFoundError('Orden', ordenId);

    const serie = await this.pg.queryOne<SerieComprobante>(
      `SELECT serie, tipo, prefijo, secuencial_actual, ruc_emisor, jurisdiccion, activa
       FROM finance.series_comprobantes WHERE tipo = $1 AND activa ORDER BY serie LIMIT 1`,
      [tipo],
    );
    if (!serie) throw new DomainError('SERIE_NO_DISPONIBLE', `No hay serie activa para ${tipo}.`);

    const jurisdiccion = await this.pg.queryOne<{ tasa_iva: string | number }>(
      `SELECT tasa_iva FROM finance.jurisdicciones WHERE codigo_pais = $1`,
      [serie.jurisdiccion],
    );
    const tasaIva = Number(jurisdiccion?.tasa_iva ?? 0.15);
    const base = Money.desdeCentavos(orden.total_cents);
    const iva = base.multiplicarPor(tasaIva);
    const total = base.sumar(iva);
    const numero = `${serie.prefijo}${String(serie.secuencial_actual + 1).padStart(8, '0')}`;
    const estado: Comprobante['estado'] = this.emitirComprobantes ? 'EMITIDO' : 'BORRADOR';

    return this.pg.transaccion(async (client) => {
      const fila = await client.query<Comprobante>(
        `INSERT INTO finance.comprobantes
          (id, serie, numero, tipo, orden_id, cliente_id, base_gravada_cents, iva_cents,
           exento_cents, total_cents, moneda, estado, datos_cliente, emitido_en)
         VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, 0, $8, 'C$', $9, $10,
                 CASE WHEN $9 = 'EMITIDO' THEN NOW() ELSE NULL END)
         RETURNING id, serie, numero, tipo, orden_id, cliente_id, base_gravada_cents, iva_cents,
                   exento_cents, total_cents, moneda, estado, datos_cliente, emitido_en, creado_en`,
        [
          serie.serie,
          numero,
          tipo,
          ordenId,
          (datosCliente as { cliente_id?: string } | undefined)?.cliente_id ?? null,
          base.centavos,
          iva.centavos,
          total.centavos,
          estado,
          datosCliente ? JSON.stringify(datosCliente) : null,
        ],
      );
      const comprobante = fila.rows[0];

      // el secuencial se consume siempre (tambien en BORRADOR) para no romper
      // la consecutividad exigida por la DGI al activar la facturacion real
      await client.query(
        `UPDATE finance.series_comprobantes SET secuencial_actual = secuencial_actual + 1 WHERE serie = $1`,
        [serie.serie],
      );

      if (estado === 'EMITIDO') {
        // Ley 822: el IVA se traslada al comprador; se registra el pasivo
        // fiscal (2.1.3 IVA por pagar) contra fondos por liquidar (2.1.2).
        await this.contabilidad.registrarEnTransaccion(client, {
          concepto: `IVA trasladado ${tipo} ${comprobante.serie} ${comprobante.numero}`,
          tipo: 'AJUSTE',
          referencia_tipo: 'comprobante',
          referencia_id: comprobante.id,
          creado_por: 'sistema',
          detalles: [
            { cuenta_codigo: '2.1.2', debe_cents: iva.centavos, haber_cents: 0, concepto: 'IVA trasladado' },
            { cuenta_codigo: '2.1.3', debe_cents: 0, haber_cents: iva.centavos, concepto: 'IVA por pagar' },
          ],
        });
        await this.outbox.insertarEnTransaccion(client, EVENTOS.COMPROBANTE_EMITIDO, {
          comprobante_id: comprobante.id,
          serie: comprobante.serie,
          numero: comprobante.numero,
          tipo,
          orden_id: ordenId,
          base_gravada_cents: base.centavos,
          iva_cents: iva.centavos,
          total_cents: total.centavos,
          moneda: 'C$',
        });
        this.logger.info({
          msg: 'Comprobante fiscal emitido',
          serie: comprobante.serie,
          numero: comprobante.numero,
          total_cents: total.centavos,
        });
      }
      return comprobante;
    });
  }

  /** Anula un comprobante (auditoria: el numero emitido no se reutiliza). */
  async anular(id: string): Promise<Comprobante | null> {
    return this.pg.queryOne<Comprobante>(
      `UPDATE finance.comprobantes SET estado = 'ANULADO'
       WHERE id = $1 AND estado IN ('BORRADOR', 'EMITIDO')
       RETURNING id, serie, numero, tipo, orden_id, cliente_id, base_gravada_cents, iva_cents,
                 exento_cents, total_cents, moneda, estado, datos_cliente, emitido_en, creado_en`,
      [id],
    );
  }

  async series(): Promise<SerieComprobante[]> {
    return this.pg.query<SerieComprobante>(
      `SELECT serie, tipo, prefijo, secuencial_actual, ruc_emisor, jurisdiccion, activa
       FROM finance.series_comprobantes ORDER BY serie`,
    );
  }

  private async consultarOrden(
    orderId: string,
  ): Promise<{ total_cents: number } | null> {
    try {
      const res = await fetch(`${this.ordersUrl}/internal/orders/${encodeURIComponent(orderId)}`, {
        headers: { 'x-internal-key': this.claveInterna },
      });
      if (!res.ok) return null;
      return (await res.json()) as { total_cents: number };
    } catch {
      return null;
    }
  }
}