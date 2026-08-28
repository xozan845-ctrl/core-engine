import { Global, Module } from '@nestjs/common';
import { PgService } from './pg.service';

/**
 * Modulo de acceso a PostgreSQL (schema propio por dominio).
 * Compatible con Supabase: SQL estandar y reproducible en Docker local (AD-08).
 */
@Global()
@Module({
  providers: [PgService],
  exports: [PgService],
})
export class PgModule {}