import { Global, Module } from '@nestjs/common';
import { EMAIL_PROVIDER, crearEmailProvider } from './email.provider';

/**
 * Correo transaccional (ADR-0003): el proveedor se resuelve una vez por
 * proceso desde el entorno. En dev/test usa `email.log`; stage/prod SMTP.
 */
@Global()
@Module({
  providers: [{ provide: EMAIL_PROVIDER, useFactory: () => crearEmailProvider() }],
  exports: [EMAIL_PROVIDER],
})
export class EmailModule {}
