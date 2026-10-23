import { Injectable } from '@nestjs/common';
import { PgService, ForbiddenError, NotFoundError } from '@core/shared';
import { UsuariosService } from '../usuarios/usuarios.service';

export type ModalidadLogistica = 'plataforma' | 'propia' | 'hibrida';
export interface Empresa { id: string; nombre: string; modalidad_logistica: ModalidadLogistica; estado: string; creado_por: string; creado_en: string }

@Injectable()
export class EmpresasService {
  constructor(private readonly pg: PgService, private readonly usuarios: UsuariosService) {}

  async crear(usuarioId: string, nombre: string, modalidad: ModalidadLogistica): Promise<Empresa> {
    return this.pg.transaccion(async (client) => {
      const result = await client.query<Empresa>(
        `INSERT INTO identity.empresas (nombre, modalidad_logistica, creado_por) VALUES ($1,$2,$3) RETURNING *`,
        [nombre.trim(), modalidad, usuarioId],
      );
      const empresa = result.rows[0];
      await client.query(
        `INSERT INTO identity.empresa_miembros (empresa_id, usuario_id, rol) VALUES ($1,$2,'empresario')`,
        [empresa.id, usuarioId],
      );
      await client.query(`UPDATE identity.usuarios SET tenant_id=$1, rol='empresario' WHERE id=$2`, [empresa.id, usuarioId]);
      return empresa;
    });
  }

  async crearVendedor(actorId: string, empresaId: string, datos: { nombre: string; correo: string; contrasena: string; participacion_porcentaje: number }) {
    const miembro = await this.pg.queryOne(`SELECT 1 FROM identity.empresa_miembros WHERE empresa_id=$1 AND usuario_id=$2 AND rol='empresario' AND estado='activo'`, [empresaId, actorId]);
    if (!miembro) throw new ForbiddenError('Solo un empresario activo puede crear vendedores.');
    const usuario = await this.usuarios.crearVendedorEnEmpresa({ ...datos, empresa_id: empresaId });
    return { ...usuario, participacion_porcentaje: datos.participacion_porcentaje };
  }

  async actualizarModalidad(
    actorId: string,
    empresaId: string,
    modalidad: ModalidadLogistica,
  ): Promise<Empresa> {
    const miembro = await this.pg.queryOne(
      `SELECT 1 FROM identity.empresa_miembros
       WHERE empresa_id=$1 AND usuario_id=$2 AND rol='empresario' AND estado='activo'`,
      [empresaId, actorId],
    );
    if (!miembro) throw new ForbiddenError('Solo un empresario activo puede cambiar la logística.');
    const empresa = await this.pg.queryOne<Empresa>(
      `UPDATE identity.empresas SET modalidad_logistica=$1
       WHERE id=$2 AND estado='activa' RETURNING *`,
      [modalidad, empresaId],
    );
    if (!empresa) throw new NotFoundError('Empresa', empresaId);
    return empresa;
  }

  async obtenerPara(usuarioId: string, empresaId: string, esGlobal: boolean): Promise<Empresa> {
    if (!esGlobal) {
      const miembro = await this.pg.queryOne(`SELECT 1 FROM identity.empresa_miembros WHERE empresa_id=$1 AND usuario_id=$2 AND estado='activo'`, [empresaId, usuarioId]);
      if (!miembro) throw new NotFoundError('Empresa', empresaId);
    }
    const empresa = await this.pg.queryOne<Empresa>(`SELECT * FROM identity.empresas WHERE id=$1`, [empresaId]);
    if (!empresa) throw new NotFoundError('Empresa', empresaId);
    return empresa;
  }

  async obtener(empresaId: string): Promise<Empresa | null> {
    return this.pg.queryOne<Empresa>(`SELECT * FROM identity.empresas WHERE id=$1`, [empresaId]);
  }
}
