import { Injectable, OnApplicationBootstrap } from '@nestjs/common';
import { UsuariosService } from '../usuarios/usuarios.service';
import { ROLES, Logger, esProduccion } from '@core/shared';

/**
 * Siembra del administrador de la bodega en el primer arranque
 * (credentiales desde variables de entorno, nunca en repositorio).
 */
@Injectable()
export class SeedService implements OnApplicationBootstrap {
  private readonly logger = Logger.create('seed');

  constructor(private readonly usuarios: UsuariosService) {}

  async onApplicationBootstrap(): Promise<void> {
    const existe = await this.usuarios.existeAdmin();
    if (existe) return;
    // Sin credenciales no se siembra nada: nunca un administrador con una
    // contrasena por defecto (esa contrasena terminaria versionada y seria
    // una puerta trasera conocida). En produccion el arranque falla.
    const email = process.env.ADMIN_EMAIL?.trim();
    const password = process.env.ADMIN_PASSWORD;
    if (!email || !password || password.length < 12) {
      const msg =
        'No se siembra el administrador inicial: define ADMIN_EMAIL y ADMIN_PASSWORD (>= 12 caracteres).';
      if (esProduccion()) throw new Error(msg);
      this.logger.error({ msg });
      return;
    }
    try {
      const admin = await this.usuarios.crear({
        nombre: 'Administrador Core Engine',
        correo: email,
        contrasena: password,
        rol: ROLES.SUPER_ADMIN,
      });
      this.logger.info({
        msg: 'Administrador inicial creado',
        email: admin.correo,
        consejo: 'cambie la contrasena en produccion',
      });
    } catch (err) {
      this.logger.error({
        msg: 'No se pudo crear el administrador inicial',
        err: err instanceof Error ? err.message : String(err),
      });
    }
  }
}
