import { Injectable } from '@nestjs/common';
import { ConflictError, ForbiddenError, PgService, Rol, ROLES } from '@core/shared';

export interface EmpresaCapacidad {
  capacidad_codigo: string;
  nombre: string;
  modulo: string;
  activo: boolean;
  config: Record<string, unknown>;
  actualizado_en: string;
}

@Injectable()
export class CapacidadesService {
  constructor(private readonly pg: PgService) {}

  async listar(empresaId: string, actor: { user_id: string; rol: Rol; tenant_id?: string }): Promise<EmpresaCapacidad[]> {
    await this.validarAcceso(empresaId, actor, false);
    return this.pg.query<EmpresaCapacidad>(
      `SELECT c.codigo AS capacidad_codigo, c.nombre, c.modulo,
              ec.activo, ec.config, ec.actualizado_en
       FROM identity.empresa_capacidades ec
       JOIN identity.capacidades c ON c.codigo = ec.capacidad_codigo
       WHERE ec.empresa_id = $1
       ORDER BY c.modulo, c.codigo`,
      [empresaId],
    );
  }

  async actualizar(
    empresaId: string,
    capacidadCodigo: string,
    activo: boolean,
    config: Record<string, unknown> | undefined,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
  ): Promise<EmpresaCapacidad> {
    await this.validarAcceso(empresaId, actor, true);
    const capacidad = await this.pg.queryOne<{ codigo: string }>(
      `SELECT c.codigo
       FROM identity.capacidades c
       JOIN identity.empresas e ON e.id = $1
       JOIN identity.vertical_capacidades vc
         ON vc.vertical_codigo = e.vertical_id
        AND vc.capacidad_codigo = c.codigo
       WHERE c.codigo = $2 AND c.activo = true`,
      [empresaId, capacidadCodigo],
    );
    if (!capacidad) throw new ConflictError(`La capacidad "${capacidadCodigo}" no existe o esta inactiva.`);
    const fila = await this.pg.queryOne<EmpresaCapacidad>(
      `INSERT INTO identity.empresa_capacidades AS ec
         (empresa_id, capacidad_codigo, activo, config, actualizado_en)
       VALUES ($1, $2, $3, COALESCE($4::jsonb, '{}'::jsonb), NOW())
       ON CONFLICT (empresa_id, capacidad_codigo) DO UPDATE
         SET activo = EXCLUDED.activo,
             config = COALESCE($4::jsonb, ec.config),
             actualizado_en = NOW()
       RETURNING capacidad_codigo, activo, config, actualizado_en,
         (SELECT nombre FROM identity.capacidades WHERE codigo = $2) AS nombre,
         (SELECT modulo FROM identity.capacidades WHERE codigo = $2) AS modulo`,
      [empresaId, capacidadCodigo, activo, config ? JSON.stringify(config) : null],
    );
    if (!fila) throw new ConflictError('No se pudo actualizar la capacidad.');
    return fila;
  }

  private async validarAcceso(
    empresaId: string,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
    escritura: boolean,
  ): Promise<void> {
    const esGlobal = actor.rol === ROLES.SUPER_ADMIN || actor.rol === ROLES.ADMIN;
    if (esGlobal) return;
    if (actor.rol !== ROLES.EMPRESARIO || actor.tenant_id !== empresaId) {
      throw new ForbiddenError('Solo el empresario activo de la empresa puede gestionar sus capacidades.');
    }
    const miembro = await this.pg.queryOne(
      `SELECT 1 FROM identity.empresa_miembros
       WHERE empresa_id = $1 AND usuario_id = $2 AND rol = 'empresario' AND estado = 'activo'`,
      [empresaId, actor.user_id],
    );
    if (!miembro) throw new ForbiddenError('El usuario no es empresario activo de esta empresa.');
    if (!escritura) return;
  }
}
