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
  'entregada',
  'cancelada',
  'devuelta',
] as const;

export type EstadoOrden = (typeof ESTADOS_ORDEN)[number];

const TRANSICIONES: Record<EstadoOrden, EstadoOrden[]> = {
  creada: ['pagada', 'cancelada'],
  pagada: ['en_preparacion', 'cancelada', 'devuelta'],
  en_preparacion: ['enviada', 'cancelada'],
  enviada: ['entregada', 'devuelta'],
  entregada: ['devuelta'],
  cancelada: [],
  devuelta: [],
};

export function puedeTransicionar(de: EstadoOrden, a: EstadoOrden): boolean {
  return TRANSICIONES[de].includes(a);
}

export function validarTransicion(de: EstadoOrden, a: EstadoOrden): void {
  if (!puedeTransicionar(de, a)) {
    throw new DomainError(
      'TRANSICION_INVALIDA',
      `Transicion invalida de estado "${de}" a "${a}".`,
    );
  }
}