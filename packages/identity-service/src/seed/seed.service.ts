import { Injectable, OnApplicationBootstrap } from '@nestjs/common';
import { UsuariosService } from '../usuarios/usuarios.service';
import { ROLES, Logger } from '@core/shared';

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
    const email = process.env.ADMIN_EMAIL ?? 'admin@core-engine.test';
    const password = process.env.ADMIN_PASSWORD ?? 'AdminCore Engine2026!';
    try {
      const admin = await this.usuarios.crear({
        nombre: 'Administrador Core Engine',
        correo: email,
        contrasena: password,
        rol: ROLES.ADMIN,
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