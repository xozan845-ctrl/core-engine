/**
 * Contratos de eventos del MVP (Tabla 22 del informe).
 * Validadas con JSON Schema en CI (seccion 6.3 / TC-10).
 */
export const EVENTOS = {
  ORDER_CREATED: 'order.created',
  STOCK_RESERVADO: 'stock.reservado',
  STOCK_FALLIDO: 'stock.fallido',
  STOCK_REINTEGRADO: 'stock.reintegrado',
  PAYMENT_PROCESADO: 'payment.procesado',
  ORDER_COMPLETADO: 'order.completado',
  COMISION_ACREDITADA: 'comision.acreditada',
  DEVOLUCION_SOLICITADA: 'devolucion.solicitada',
  SHIPMENT_STARTED: 'shipment.started',
  STOCK_UPDATED: 'stock.updated',
  ORDER_STATUS_UPDATED: 'order.status.updated',
  ASIENTO_REGISTRADO: 'asiento.registrado',
  DECLARACION_GENERADA: 'declaracion.generada',
  COMPROBANTE_EMITIDO: 'comprobante.emitido',
  /** Emitido cuando una venta incluye contexto geoespacial/demografico opcional. */
  VENTA_GEOLOCALIZADA: 'venta.geolocalizada',
} as const;

export type NombreEvento = (typeof EVENTOS)[keyof typeof EVENTOS];

/** Evento base: schema JSON del mensaje publicado en el bus. */
export interface EventoBus<T = unknown> {
  event_id: string;
  tipo: NombreEvento;
  ocurrido_en: string; // ISO 8601
  /** ID de correlacion de la peticion que origino el evento (doc 5.3: trazabilidad distribuida). */
  request_id?: string;
  data: T;
}

export interface ItemOrden {
  oferta_id: string;
  sku: string;
  producto_nombre: string;
  cantidad: number;
  precio_unitario_cents: number; // precio de venta final (RN-01)
  vendedor_id: string;
  tienda_id: string;
}

export interface OrdenData {
  order_id: string;
  cliente_id: string;
  items: ItemOrden[];
  total_cents: number;
  estado: 'creada';
}

export interface StockReservadoData {
  order_id: string;
  items: { sku: string; cantidad: number }[];
}

export interface StockFallidoData {
  order_id: string;
  items: { sku: string; cantidad: number; motivo: string }[];
}

export interface StockReintegradoData {
  order_id: string;
  items: { sku: string; cantidad: number }[];
}

export interface StockUpdatedData {
  order_id: string;
  tipo: 'reservado' | 'reintegrado' | 'ajuste';
  /** Stock restante por SKU tras la operacion (RN-02: oferta -> agotada en stock 0). */
  items: { sku: string; stock_restante: number }[];
}

export interface PaymentProcesadoData {
  order_id: string;
  monto_cents: number;
  estado: 'procesado';
  metodo: 'simulado';
}

export interface OrderCompletadoData {
  order_id: string;
  entregado_en: string;
}

export interface ComisionAcreditadaData {
  order_id: string;
  monto_cents: number; // comision de la plataforma (RN-04)
  monto_vendedor_cents: number;
  vendedor_id: string;
}

export interface DevolucionSolicitadaData {
  order_id: string;
  motivo: string;
  items: { sku: string; cantidad: number }[];
}

export interface OrderStatusUpdatedData {
  order_id: string;
  estado: string;
  previo_estado: string;
  motivo?: string;
}

/** Devolución: monto devuelto (compra) y comisión revertida (RN-06). */
export interface DevolucionSolicitadaDataCompleta extends DevolucionSolicitadaData {
  monto_devolucion_cents?: number;
  comision_devolucion_cents?: number;
}

export interface AsientoRegistradoData {
  asiento_id: string;
  fecha: string; // ISO 8601
  tipo: string;
  concepto: string;
  referencia_tipo: string;
  referencia_id?: string;
  debe_cents: number;
  haber_cents: number;
  moneda: string;
}

export interface DeclaracionGeneradaData {
  declaracion_id: string;
  jurisdiccion: string;
  tipo: 'IVA' | 'IR' | 'CUOTA_FIJA';
  periodo_inicio: string;
  periodo_fin: string;
  a_pagar_cents: number;
  estado: string;
}

export interface ComprobanteEmitidoData {
  comprobante_id: string;
  serie: string;
  numero: string;
  tipo: 'FACTURA' | 'NOTA_CREDITO';
  orden_id?: string;
  base_gravada_cents: number;
  iva_cents: number;
  total_cents: number;
  moneda: string;
}

/**
 * Contexto geoespacial y demografico de una venta (inteligencia de mercado).
 * Las coordenadas son completas para precision en mapas de calor.
 * La demografia usa rangos anonimos para ciencias de datos sin vincular a PII exacta.
 */
export interface VentaGeolocalizadaData {
  order_id: string;
  vendedor_id: string;
  /** SKUs vendidos (uno por item) — permite analisis por producto/zona. */
  skus: string[];
  monto_cents: number;
  /** Coordenadas GPS completas del punto de venta (modelo Ubicacion del app-test). */
  lat: number;
  lng: number;
  /** Precision del GPS en metros. */
  precision?: number;
  /** Velocidad en m/s en el momento del registro. */
  velocidad?: number;
  /** Rumbo en grados (0-360). */
  rumbo?: number;
  /** Rango de edad anonimizado (ciencias de datos, no PII exacta). */
  rango_edad?: '18-24' | '25-34' | '35-44' | '45-54' | '55+';
  /** Genero en rango anonimo. NS = No Especificado. */
  genero?: 'M' | 'F' | 'NS';
  /** Tipo de actividad del app-test. */
  tipo_actividad?: 'impulsacion' | 'venta' | 'entrega' | 'reparto';
  /** Resultado de visita al cliente (del app-test). */
  resultado_visita?: 'visitado' | 'cerca_no_visitado' | 'no_visitado';
  /** Distancia al cliente en metros (calculada en el frontend con Haversine). */
  distancia_cliente_metros?: number;
}

/** Mapa tipo -> contrato valido (validacion simple en CI, Pact en staging). */
export const CONTRATOS: Record<NombreEvento, { editor: string; consumidores: string[] }> = {
  [EVENTOS.ORDER_CREATED]: {
    editor: 'Pedidos',
    consumidores: ['Inventario', 'Comisiones', 'Notificaciones'],
  },
  [EVENTOS.STOCK_RESERVADO]: {
    editor: 'Pedidos',
    consumidores: ['Pedidos (confirmacion)'],
  },
  [EVENTOS.STOCK_FALLIDO]: {
    editor: 'Pedidos',
    consumidores: ['Pedidos (rechazo)'],
  },
  [EVENTOS.STOCK_REINTEGRADO]: {
    editor: 'Inventario',
    consumidores: ['Pedidos', 'Notificaciones'],
  },
  [EVENTOS.PAYMENT_PROCESADO]: {
    editor: 'Pagos',
    consumidores: ['Logistica (prepara envio)'],
  },
  [EVENTOS.ORDER_COMPLETADO]: {
    editor: 'Logistica',
    consumidores: ['Comisiones (devenga comision)'],
  },
  [EVENTOS.COMISION_ACREDITADA]: {
    editor: 'Comisiones',
    consumidores: ['Liquidaciones (corte quincenal)'],
  },
  [EVENTOS.DEVOLUCION_SOLICITADA]: {
    editor: 'Devoluciones',
    consumidores: ['Pedidos (retorno)', 'Pagos (reembolso)'],
  },
  [EVENTOS.SHIPMENT_STARTED]: {
    editor: 'Logistica',
    consumidores: ['Notificaciones'],
  },
  [EVENTOS.STOCK_UPDATED]: {
    editor: 'Inventario',
    consumidores: ['Notificaciones'],
  },
  [EVENTOS.ORDER_STATUS_UPDATED]: {
    editor: 'Pedidos',
    consumidores: ['Logistica', 'Notificaciones'],
  },
  [EVENTOS.ASIENTO_REGISTRADO]: {
    editor: 'Finanzas y Contabilidad',
    consumidores: ['Auditoria', 'Notificaciones'],
  },
  [EVENTOS.DECLARACION_GENERADA]: {
    editor: 'Finanzas y Contabilidad',
    consumidores: ['Notificaciones'],
  },
  [EVENTOS.COMPROBANTE_EMITIDO]: {
    editor: 'Finanzas y Contabilidad',
    consumidores: ['Notificaciones'],
  },
  [EVENTOS.VENTA_GEOLOCALIZADA]: {
    editor: 'Orders / Field (app-test)',
    consumidores: ['Inteligencia de Mercado'],
  },
};