import { SetMetadata } from '@nestjs/common';
import { Capacidad } from './capability.constants';

export const CAPABILITIES_KEY = 'capacidades_requeridas';

/** Marca una operación con todas las capacidades que debe tener activas. */
export const RequiereCapacidades = (...capacidades: Capacidad[]) =>
  SetMetadata(CAPABILITIES_KEY, capacidades);
