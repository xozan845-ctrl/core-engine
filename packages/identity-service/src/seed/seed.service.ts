import { Injectable, OnApplicationBootstrap } from '@nestjs/common';
import { UsuariosService } from '../usuarios/usuarios.service';
import {
  ROLES,
  Logger,
  PgService,
  emailAdminInicial,
  passwordAdminInicial,
} from '@core/shared';

/**
 * Siembra del administrador de la bodega en el primer arranque
 * (credentiales desde variables de entorno, nunca en repositorio).
 */
@Injectable()
export class SeedService implements OnApplicationBootstrap {
  private readonly logger = Logger.create('seed');

  constructor(
    private readonly usuarios: UsuariosService,
    private readonly pg: PgService,
  ) {}

  async onApplicationBootstrap(): Promise<void> {
    await this.sembrarAdmin();
    await this.sembrarTenantPlataforma();
  }

  private async sembrarAdmin(): Promise<void> {
    const existe = await this.usuarios.existeAdmin();
    if (existe) return;
    const email = emailAdminInicial();
    const password = passwordAdminInicial();
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

  /**
   * Tenant de la flota global de la plataforma (PLATFORM_LOGISTICS_TENANT_ID).
   * field-service exige `operacion_campo` (R-AR-5); el tenant es un registro
   * real de empresa para que la capacidad exista. Idempotente en cada arranque.
   */
  private async sembrarTenantPlataforma(): Promise<void> {
    const id = (process.env.PLATFORM_LOGISTICS_TENANT_ID ?? '').trim()
      || '00000000-0000-4000-8000-000000000001';
    if (!/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(id)) {
      this.logger.warn({ msg: 'PLATFORM_LOGISTICS_TENANT_ID no es un UUID v4; se omite', id });
      return;
    }
    try {
      const admin = await this.pg.queryOne<{ id: string }>(
        `SELECT id FROM identity.usuarios WHERE rol = 'super_admin' ORDER BY creado_en ASC LIMIT 1`,
      );
      if (!admin) return;
      await this.pg.query(
        `INSERT INTO identity.empresas
           (id, nombre, slug, vertical_id, pack_codigo, pack_version, creado_por)
         VALUES ($1, 'Plataforma logística', 'plataforma-logistica', 'flota', 'pack_flota', 1, $2)
         ON CONFLICT DO NOTHING`,
        [id, admin.id],
      );
      await this.pg.query(
        `INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
         SELECT $1, capacidad_codigo, config, activo
           FROM identity.pack_capacidades
          WHERE pack_codigo = 'pack_flota' AND pack_version = 1
         ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING`,
        [id],
      );
    } catch (err) {
      this.logger.warn({
        msg: 'No se pudo asegurar el tenant de plataforma',
        err: err instanceof Error ? err.message : String(err),
      });
    }
  }
}
