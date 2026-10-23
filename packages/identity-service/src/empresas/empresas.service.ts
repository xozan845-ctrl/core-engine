import { Injectable } from '@nestjs/common';
import {
  PgService,
  ForbiddenError,
  NotFoundError,
  ConflictError,
  esUuid,
  esViolacionUnicidad,
  slugConSufijo,
  slugDesdeNombre,
  validarSlugSolicitado,
} from '@core/shared';
import { UsuariosService } from '../usuarios/usuarios.service';

export type ModalidadLogistica = 'plataforma' | 'propia' | 'hibrida';
export interface Empresa {
  id: string;
  nombre: string;
  slug: string;
  modalidad_logistica: ModalidadLogistica;
  estado: string;
  creado_por: string;
  creado_en: string;
}

interface EmpresaConEstadoSlug extends Empresa {
  es_actual: boolean;
}

export interface EmpresaResuelta {
  empresa: Empresa;
  /** Ruta a la que redirigir cuando se consultó un slug anterior. */
  redirigirA?: string;
}

interface ClienteSlug {
  query(texto: string, valores?: unknown[]): Promise<{ rowCount: number | null }>;
}

const MAX_INTENTOS_SLUG = 50;

@Injectable()
export class EmpresasService {
  constructor(private readonly pg: PgService, private readonly usuarios: UsuariosService) {}

  async crear(
    usuarioId: string,
    nombre: string,
    modalidad: ModalidadLogistica,
    slugSolicitado?: string,
  ): Promise<Empresa> {
    const nombreFinal = nombre.trim();
    const slugExplicito =
      slugSolicitado === undefined ? undefined : validarSlugSolicitado(slugSolicitado);
    // Un slug explicito no se renombra en silencio: si esta ocupado es 409.
    // Uno derivado del nombre se resuelve con sufijos y se reintenta si otra
    // transaccion gana la carrera.
    const maxIntentos = slugExplicito ? 1 : 3;

    for (let intento = 1; intento <= maxIntentos; intento += 1) {
      try {
        return await this.pg.transaccion(async (client) => {
          const slug = await this.elegirSlug(client, nombreFinal, slugExplicito);
          const result = await client.query<Empresa>(
            `INSERT INTO identity.empresas (nombre, slug, modalidad_logistica, creado_por)
             VALUES ($1,$2,$3,$4) RETURNING *`,
            [nombreFinal, slug, modalidad, usuarioId],
          );
          const empresa = result.rows[0];
          await client.query(
            `INSERT INTO identity.empresa_miembros (empresa_id, usuario_id, rol) VALUES ($1,$2,'empresario')`,
            [empresa.id, usuarioId],
          );
          await client.query(`UPDATE identity.usuarios SET tenant_id=$1, rol='empresario' WHERE id=$2`, [empresa.id, usuarioId]);
          return empresa;
        });
      } catch (error) {
        if (!esViolacionUnicidad(error)) throw error;
        if (slugExplicito || intento === maxIntentos) {
          throw new ConflictError(`El slug "${slugExplicito ?? nombreFinal}" ya está en uso.`);
        }
      }
    }

    throw new ConflictError('No se pudo asignar un slug disponible.');
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

  /**
   * Resuelve una empresa por UUID o por slug (actual o anterior, ADR-26).
   * La autorizacion se aplica despues de resolver para no revelar la
   * existencia de empresas ajenas.
   */
  async resolverPara(
    usuarioId: string,
    identificador: string,
    esGlobal: boolean,
    rolMiembro?: 'empresario' | 'vendedor',
  ): Promise<EmpresaResuelta> {
    const fila = await this.buscarPorIdentificador(identificador);
    if (!fila) throw new NotFoundError('Empresa', identificador);

    if (!esGlobal) {
      const miembro = rolMiembro
        ? await this.pg.queryOne(
            `SELECT 1 FROM identity.empresa_miembros
             WHERE empresa_id=$1 AND usuario_id=$2 AND rol=$3 AND estado='activo'`,
            [fila.id, usuarioId, rolMiembro],
          )
        : await this.pg.queryOne(
            `SELECT 1 FROM identity.empresa_miembros
             WHERE empresa_id=$1 AND usuario_id=$2 AND estado='activo'`,
            [fila.id, usuarioId],
          );
      if (!miembro) throw new NotFoundError('Empresa', identificador);
    }

    const { es_actual, ...empresa } = fila;
    return {
      empresa,
      redirigirA: es_actual ? undefined : `/api/v1/empresas/${empresa.slug}`,
    };
  }

  async obtenerPara(usuarioId: string, empresaId: string, esGlobal: boolean): Promise<Empresa> {
    return (await this.resolverPara(usuarioId, empresaId, esGlobal)).empresa;
  }

  /** Cambia el slug actual; el anterior queda en el historial y redirige 301. */
  async actualizarSlug(
    usuarioId: string,
    identificador: string,
    slugSolicitado: string,
  ): Promise<Empresa> {
    // El controlador ya exige rol empresario; aqui se comprueba ademas que la
    // membresia activa en ESA empresa sea de empresario (no basta el rol JWT).
    const { empresa } = await this.resolverPara(usuarioId, identificador, false, 'empresario');
    const slug = validarSlugSolicitado(slugSolicitado);
    if (slug === empresa.slug) return empresa;

    const ocupado = await this.pg.queryOne(
      `SELECT 1 FROM identity.empresa_slugs WHERE slug=$1`,
      [slug],
    );
    if (ocupado) throw new ConflictError(`El slug "${slug}" ya está en uso.`);

    try {
      const actualizada = await this.pg.queryOne<Empresa>(
        `UPDATE identity.empresas SET slug=$1 WHERE id=$2 RETURNING *`,
        [slug, empresa.id],
      );
      if (!actualizada) throw new NotFoundError('Empresa', empresa.id);
      return actualizada;
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError(`El slug "${slug}" ya está en uso.`);
      }
      throw error;
    }
  }

  async obtener(empresaId: string): Promise<Empresa | null> {
    return this.pg.queryOne<Empresa>(`SELECT * FROM identity.empresas WHERE id=$1`, [empresaId]);
  }

  private async buscarPorIdentificador(
    identificador: string,
  ): Promise<EmpresaConEstadoSlug | null> {
    if (esUuid(identificador)) {
      return this.pg.queryOne<EmpresaConEstadoSlug>(
        `SELECT *, true AS es_actual FROM identity.empresas WHERE id=$1`,
        [identificador],
      );
    }
    return this.pg.queryOne<EmpresaConEstadoSlug>(
      `SELECT e.*, (s.reemplazado_en IS NULL) AS es_actual
       FROM identity.empresa_slugs s
       JOIN identity.empresas e ON e.id = s.empresa_id
       WHERE s.slug = $1`,
      [identificador],
    );
  }

  private async elegirSlug(
    cliente: ClienteSlug,
    nombre: string,
    explicito?: string,
  ): Promise<string> {
    if (explicito) {
      if (!(await this.slugLibre(cliente, explicito))) {
        throw new ConflictError(`El slug "${explicito}" ya está en uso.`);
      }
      return explicito;
    }

    const base = slugDesdeNombre(nombre, 'empresa');
    for (let intento = 1; intento <= MAX_INTENTOS_SLUG; intento += 1) {
      const candidato = intento === 1 ? base : slugConSufijo(base, intento);
      if (await this.slugLibre(cliente, candidato)) return candidato;
    }
    return this.slugAleatorio(base);
  }

  private async slugLibre(cliente: ClienteSlug, slug: string): Promise<boolean> {
    const fila = await cliente.query(
      `SELECT 1 FROM identity.empresa_slugs WHERE slug=$1`,
      [slug],
    );
    return !fila.rowCount;
  }

  private slugAleatorio(base: string): string {
    const sufijo = `-${Date.now().toString(36)}`;
    const maximo = Math.max(2, 80 - sufijo.length);
    const recorte = base.slice(0, maximo).replace(/-+$/g, '');
    return `${recorte || 'empresa'}${sufijo}`;
  }
}
