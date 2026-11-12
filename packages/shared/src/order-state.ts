import { DomainError } from './errors';

/**
 * Ciclo de vida de una orden (Tabla 13).
 * Creada -> Pagada -> En preparacion -> Enviada -> Entregada
 * Creada -> Cancelada / cualquier estado activo -> Cancelada | Devuelta
 */
export const ESTADOS_ORDEN = [
  'creada',
  'pagada',
  'en_preparacion',
  'enviada',
  // ciclo propio de cotizaciones (enviar -> aceptar/rechazar)
  'aceptada',
  'rechazada',
  'entregada',
  'cancelada',
  'devuelta',
] as const;

export type EstadoOrden = (typeof ESTADOS_ORDEN)[number];

export const TIPOS_ORDEN = ['compra', 'reserva', 'cotizacion', 'orden_servicio'] as const;
export type TipoOrden = (typeof TIPOS_ORDEN)[number];

export const FULFILLMENTS = ['delivery', 'pickup', 'onsite', 'none'] as const;
export type Fulfillment = (typeof FULFILLMENTS)[number];

const TRANSICIONES: Record<EstadoOrden, EstadoOrden[]> = {
  creada: ['pagada', 'cancelada'],
  pagada: ['en_preparacion', 'cancelada', 'devuelta'],
  en_preparacion: ['enviada', 'cancelada'],
  enviada: ['entregada', 'devuelta'],
  aceptada: [],
  rechazada: [],
  entregada: ['devuelta'],
  cancelada: [],
  devuelta: [],
};

const TRANSICIONES_COTIZACION: Record<EstadoOrden, EstadoOrden[]> = {
  creada: ['enviada', 'cancelada'],
  enviada: ['aceptada', 'rechazada', 'cancelada'],
  aceptada: [],
  rechazada: [],
  pagada: [],
  en_preparacion: [],
  entregada: [],
  cancelada: [],
  devuelta: [],
};

const TRANSICIONES_POR_TIPO: Record<TipoOrden, Record<EstadoOrden, EstadoOrden[]>> = {
  compra: TRANSICIONES,
  reserva: {
    creada: ['pagada', 'cancelada'],
    pagada: ['entregada', 'cancelada', 'devuelta'],
    en_preparacion: [],
    enviada: [],
    aceptada: [],
    rechazada: [],
    entregada: ['devuelta'],
    cancelada: [],
    devuelta: [],
  },
  cotizacion: TRANSICIONES_COTIZACION,
  orden_servicio: {
    creada: ['pagada', 'cancelada'],
    pagada: ['en_preparacion', 'cancelada', 'devuelta'],
    en_preparacion: ['entregada', 'cancelada'],
    enviada: [],
    aceptada: [],
    rechazada: [],
    entregada: ['devuelta'],
    cancelada: [],
    devuelta: [],
  },
};

export function validarTransicionPorTipo(tipo: TipoOrden, de: EstadoOrden, a: EstadoOrden): void {
  if (!TRANSICIONES_POR_TIPO[tipo][de].includes(a)) {
    throw new DomainError(
      'TRANSICION_INVALIDA',
      `Transicion invalida de estado "${de}" a "${a}" para orden "${tipo}".`,
    );
  }
}
