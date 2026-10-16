import { Module } from '@nestjs/common';
import { ALMACEN_OBJETOS } from '@core/shared';
import { MinioAlmacen } from './minio.almacen';
import { STORAGE_CONFIG, leerConfig } from '../config/storage.config';

/**
 * Cablea el puerto `AlmacenObjetos` (R-SB-6) con la implementacion MinIO. El
 * resto del servicio inyecta `ALMACEN_OBJETOS`, nunca el SDK.
 */
@Module({
  providers: [
    { provide: STORAGE_CONFIG, useFactory: () => leerConfig() },
    MinioAlmacen,
    { provide: ALMACEN_OBJETOS, useExisting: MinioAlmacen },
  ],
  exports: [ALMACEN_OBJETOS, STORAGE_CONFIG],
})
export class AlmacenModule {}
