/** Capacidades estables de la plataforma multi-vertical (ADR-0001). */
export const CAPACIDADES = {
  CATALOGO_ITEMS: 'catalogo_items',
  INVENTARIO_STOCK: 'inventario_stock',
  INVENTARIO_LOTES: 'inventario_lotes',
  CODIGO_BARRAS: 'codigo_barras',
  AGENDA_RECURSOS: 'agenda_recursos',
  HISTORIA_CLINICA: 'historia_clinica',
  RECETAS: 'recetas',
  REGISTROS: 'registros',
  FULFILLMENT_DELIVERY: 'fulfillment_delivery',
  FULFILLMENT_PICKUP: 'fulfillment_pickup',
  FULFILLMENT_ONSITE: 'fulfillment_onsite',
  FULFILLMENT_DIGITAL: 'fulfillment_digital',
  LOGISTICA_RUTAS: 'logistica_rutas',
  OPERACION_CAMPO: 'operacion_campo',
  INTELIGENCIA_MERCADO: 'inteligencia_mercado',
  COTIZACIONES: 'cotizaciones',
  VENTAS_CREDITO: 'ventas_credito',
  FACTURACION: 'facturacion',
  LIQUIDACIONES: 'liquidaciones',
  CONTRATOS: 'contratos',
  SUSCRIPCIONES: 'suscripciones',
  MESAS: 'mesas',
} as const;

export type Capacidad = (typeof CAPACIDADES)[keyof typeof CAPACIDADES];
