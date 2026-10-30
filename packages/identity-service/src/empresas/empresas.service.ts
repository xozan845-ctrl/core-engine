import { Injectable } from '@nestjs/common';
import {
  PgService,
  ForbiddenError,
  DomainError,
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
/**
 * Tipo de empresa (ADR-27). Solo las comerciales tienen catálogo propio:
 * la logística transporta y la plataforma agrupa empresas técnicas/comodín.
 */
export type TipoEmpresa = 'comercial' | 'logistica' | 'plataforma';
export interface Empresa {
  id: string;
  nombre: string;
  slug: string;
  modalidad_logistica: ModalidadLogistica;
  tipo_empresa: TipoEmpresa;
  estado: string;
  creado_por: string;
  creado_en: string;
  /** Perfil comercial y fiscal (R-PS-01). */
  ruc: string;
  razon_social: string;
  nombre_comercial: string;
  descripcion: string;
  telefono: string;
  correo_contacto: string;
  sitio_web: string;
  direccion: string;
  ciudad: string;
  departamento: string;
  pais: string;
  moneda: string;
  zona_horaria: string;
  sector: string;
  representante_legal: string;
  horario_atencion: string;
  logo_objeto_id: string | null;
  /** URL pública derivada del objeto de storage; no se persiste. */
  logo_url?: string | null;
  actualizado_en: string;
}

/** Campos editables del perfil de empresa; todos opcionales. */
export interface PerfilEmpresaInput {
  ruc?: string;
  razon_social?: string;
  nombre_comercial?: string;
  descripcion?: string;
  telefono?: string;
  correo_contacto?: string;
  sitio_web?: string;
  direccion?: string;
  ciudad?: string;
  departamento?: string;
  pais?: string;
  moneda?: string;
  zona_horaria?: string;
  sector?: string;
  representante_legal?: string;
  horario_atencion?: string;
  logo_objeto_id?: string | null;
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

type PerfilNormalizado = Required<Omit<PerfilEmpresaInput, 'logo_objeto_id'>> & {
  logo_objeto_id: string | null;
};

interface ObjetoStorage {
  id: string;
  tenant_id: string;
  estado: string;
  publico: boolean;
  content_type: string;
}

const MAX_INTENTOS_SLUG = 50;

@Injectable()
export class EmpresasService {
  private readonly storageUrl = process.env.STORAGE_SERVICE_URL ?? 'http://storage-service:3010';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(private readonly pg: PgService, private readonly usuarios: UsuariosService) {}

  async crear(
    usuarioId: string,
    nombre: string,
    modalidad: ModalidadLogistica,
    slugSolicitado?: string,
    perfil?: PerfilEmpresaInput,
    tipo: TipoEmpresa = 'comercial',
  ): Promise<Empresa> {
    if (!['comercial', 'logistica', 'plataforma'].includes(tipo)) {
      throw new DomainError('TIPO_EMPRESA_INVALIDO', 'El tipo de empresa no es válido.');
    }
    const nombreFinal = nombre.trim();
    const meta = this.normalizarPerfil({ ...(perfil ?? {}), logo_objeto_id: null });
    this.validarPerfil(meta);
    await this.validarRucUnico(meta.ruc);
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
            `INSERT INTO identity.empresas
               (nombre, slug, modalidad_logistica, creado_por, ruc, razon_social,
                nombre_comercial, descripcion, telefono, correo_contacto, sitio_web,
                direccion, ciudad, departamento, pais, moneda, zona_horaria, sector,
                representante_legal, horario_atencion, logo_objeto_id, tipo_empresa)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,NULL,$21)
             RETURNING *`,
            [
              nombreFinal,
              slug,
              modalidad,
              usuarioId,
              meta.ruc,
              meta.razon_social,
              meta.nombre_comercial,
              meta.descripcion,
              meta.telefono,
              meta.correo_contacto,
              meta.sitio_web,
              meta.direccion,
              meta.ciudad,
              meta.departamento,
              meta.pais,
              meta.moneda,
              meta.zona_horaria,
              meta.sector,
              meta.representante_legal,
              meta.horario_atencion,
              tipo,
            ],
          );
          const empresa = result.rows[0];
          await client.query(
            `INSERT INTO identity.empresa_miembros (empresa_id, usuario_id, rol) VALUES ($1,$2,'empresario')`,
            [empresa.id, usuarioId],
          );
          await client.query(`UPDATE identity.usuarios SET tenant_id=$1, rol='empresario' WHERE id=$2`, [empresa.id, usuarioId]);
          return this.conLogoUrl(empresa);
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
    return this.conLogoUrl(empresa);
  }

  /**
   * ADR-27: solo un administrador global clasifica una empresa. Cambiar a
   * 'logistica' la excluye de los catálogos; los productos existentes dejan de
   * ser publicables sin borrar datos.
   */
  async actualizarTipo(
    empresaId: string,
    tipo: TipoEmpresa,
    actorId?: string,
    esGlobal = false,
  ): Promise<Empresa> {
    if (!['comercial', 'logistica', 'plataforma'].includes(tipo)) {
      throw new DomainError('TIPO_EMPRESA_INVALIDO', 'El tipo de empresa no es válido.');
    }
    if (!esGlobal) {
      if (!actorId) throw new ForbiddenError('Se requiere un empresario activo.');
      const miembro = await this.pg.queryOne(
        `SELECT 1 FROM identity.empresa_miembros
         WHERE empresa_id=$1 AND usuario_id=$2 AND rol='empresario' AND estado='activo'`,
        [empresaId, actorId],
      );
      if (!miembro) {
        throw new ForbiddenError('Solo un empresario activo puede cambiar el tipo de empresa.');
      }
    }
    const empresa = await this.pg.queryOne<Empresa>(
      `UPDATE identity.empresas SET tipo_empresa=$1 WHERE id=$2 RETURNING *`,
      [tipo, empresaId],
    );
    if (!empresa) throw new NotFoundError('Empresa', empresaId);
    return this.conLogoUrl(empresa);
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
      empresa: this.conLogoUrl(empresa),
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
    if (slug === empresa.slug) return this.conLogoUrl(empresa);

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
      return this.conLogoUrl(actualizada);
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError(`El slug "${slug}" ya está en uso.`);
      }
      throw error;
    }
  }

  async obtener(empresaId: string): Promise<Empresa | null> {
    const empresa = await this.pg.queryOne<Empresa>(
      `SELECT * FROM identity.empresas WHERE id=$1`,
      [empresaId],
    );
    return empresa ? this.conLogoUrl(empresa) : null;
  }

  /**
   * Actualiza el perfil comercial/fiscal de forma parcial. El empresario solo
   * puede editar su empresa; admin/super_admin pueden editar cualquiera.
   */
  async actualizarPerfil(
    usuarioId: string,
    identificador: string,
    cambios: PerfilEmpresaInput,
    esGlobal = false,
  ): Promise<Empresa> {
    const { empresa } = await this.resolverPara(
      usuarioId,
      identificador,
      esGlobal,
      esGlobal ? undefined : 'empresario',
    );
    const meta = this.normalizarPerfil(cambios, empresa);
    this.validarPerfil(meta);
    if (meta.ruc !== empresa.ruc) await this.validarRucUnico(meta.ruc, empresa.id);
    if (meta.logo_objeto_id && meta.logo_objeto_id !== empresa.logo_objeto_id) {
      await this.validarLogo(empresa.id, meta.logo_objeto_id);
    }
    try {
      const actualizada = await this.pg.queryOne<Empresa>(
        `UPDATE identity.empresas SET
           ruc=$2, razon_social=$3, nombre_comercial=$4, descripcion=$5,
           telefono=$6, correo_contacto=$7, sitio_web=$8, direccion=$9,
           ciudad=$10, departamento=$11, pais=$12, moneda=$13, zona_horaria=$14,
           sector=$15, representante_legal=$16, horario_atencion=$17,
           logo_objeto_id=$18, actualizado_en=NOW()
         WHERE id=$1 RETURNING *`,
        [
          empresa.id,
          meta.ruc,
          meta.razon_social,
          meta.nombre_comercial,
          meta.descripcion,
          meta.telefono,
          meta.correo_contacto,
          meta.sitio_web,
          meta.direccion,
          meta.ciudad,
          meta.departamento,
          meta.pais,
          meta.moneda,
          meta.zona_horaria,
          meta.sector,
          meta.representante_legal,
          meta.horario_atencion,
          meta.logo_objeto_id,
        ],
      );
      if (!actualizada) throw new NotFoundError('Empresa', empresa.id);
      return this.conLogoUrl(actualizada);
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError('El RUC ya está registrado en otra empresa.');
      }
      throw error;
    }
  }

  private normalizarPerfil(
    datos: PerfilEmpresaInput,
    actual?: Empresa,
  ): PerfilNormalizado {
    const texto = (valor: unknown, respaldo = ''): string =>
      typeof valor === 'string' ? valor.trim() : respaldo;
    return {
      ruc: texto(datos.ruc, actual?.ruc ?? ''),
      razon_social: texto(datos.razon_social, actual?.razon_social ?? ''),
      nombre_comercial: texto(datos.nombre_comercial, actual?.nombre_comercial ?? ''),
      descripcion: texto(datos.descripcion, actual?.descripcion ?? ''),
      telefono: texto(datos.telefono, actual?.telefono ?? ''),
      correo_contacto: texto(datos.correo_contacto, actual?.correo_contacto ?? ''),
      sitio_web: texto(datos.sitio_web, actual?.sitio_web ?? ''),
      direccion: texto(datos.direccion, actual?.direccion ?? ''),
      ciudad: texto(datos.ciudad, actual?.ciudad ?? ''),
      departamento: texto(datos.departamento, actual?.departamento ?? ''),
      pais: texto(datos.pais, actual?.pais ?? 'Nicaragua') || 'Nicaragua',
      moneda: (texto(datos.moneda, actual?.moneda ?? 'NIO') || 'NIO').toUpperCase(),
      zona_horaria: texto(datos.zona_horaria, actual?.zona_horaria ?? 'America/Managua') ||
        'America/Managua',
      sector: texto(datos.sector, actual?.sector ?? ''),
      representante_legal: texto(datos.representante_legal, actual?.representante_legal ?? ''),
      horario_atencion: texto(datos.horario_atencion, actual?.horario_atencion ?? ''),
      logo_objeto_id:
        datos.logo_objeto_id !== undefined
          ? datos.logo_objeto_id
          : (actual?.logo_objeto_id ?? null),
    };
  }

  private validarPerfil(meta: PerfilNormalizado): void {
    const limites: [string, number, string][] = [
      ['ruc', 30, meta.ruc],
      ['razon_social', 160, meta.razon_social],
      ['nombre_comercial', 160, meta.nombre_comercial],
      ['descripcion', 1000, meta.descripcion],
      ['telefono', 40, meta.telefono],
      ['sitio_web', 200, meta.sitio_web],
      ['direccion', 300, meta.direccion],
      ['ciudad', 80, meta.ciudad],
      ['departamento', 80, meta.departamento],
      ['pais', 80, meta.pais],
      ['zona_horaria', 64, meta.zona_horaria],
      ['sector', 80, meta.sector],
      ['representante_legal', 160, meta.representante_legal],
      ['horario_atencion', 120, meta.horario_atencion],
    ];
    for (const [campo, maximo, valor] of limites) {
      if (valor.length > maximo) {
        throw new DomainError('PERFIL_INVALIDO', `${campo} supera ${maximo} caracteres.`);
      }
    }
    if (!/^[A-Z]{3}$/.test(meta.moneda)) {
      throw new DomainError('PERFIL_INVALIDO', 'La moneda debe ser un código ISO de 3 letras.');
    }
    if (meta.correo_contacto && !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(meta.correo_contacto)) {
      throw new DomainError('PERFIL_INVALIDO', 'El correo de contacto no es válido.');
    }
    if (meta.logo_objeto_id !== null && !esUuid(meta.logo_objeto_id)) {
      throw new DomainError('PERFIL_INVALIDO', 'El logo debe ser un objeto de storage válido.');
    }
  }

  private async validarRucUnico(ruc: string, excluirId?: string): Promise<void> {
    if (!ruc) return;
    const params: unknown[] = [ruc];
    let where = 'upper(ruc) = upper($1)';
    if (excluirId) {
      params.push(excluirId);
      where += ' AND id <> $2';
    }
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM identity.empresas WHERE ${where} LIMIT 1`,
      params,
    );
    if (fila) throw new ConflictError('El RUC ya está registrado en otra empresa.');
  }

  /** El logo debe ser una imagen pública y activa del mismo tenant. */
  private async validarLogo(empresaId: string, objetoId: string): Promise<void> {
    let respuesta: Response;
    try {
      respuesta = await fetch(
        `${this.storageUrl}/internal/storage/objetos/${encodeURIComponent(objetoId)}` +
          `?tenant_id=${encodeURIComponent(empresaId)}`,
        { headers: { 'x-internal-key': this.claveInterna } },
      );
    } catch {
      throw new DomainError('LOGO_NO_VERIFICABLE', 'No se pudo verificar el logo.');
    }
    if (respuesta.status === 404) throw new NotFoundError('Logo', objetoId);
    if (!respuesta.ok) {
      throw new DomainError('LOGO_NO_VERIFICABLE', 'No se pudo verificar el logo.');
    }
    const objeto = (await respuesta.json()) as ObjetoStorage;
    if (
      objeto.tenant_id !== empresaId ||
      objeto.estado !== 'activo' ||
      objeto.publico !== true ||
      !String(objeto.content_type).startsWith('image/')
    ) {
      throw new DomainError(
        'LOGO_INVALIDO',
        'El logo debe ser una imagen pública y activa de la empresa.',
      );
    }
  }

  private conLogoUrl(empresa: Empresa): Empresa {
    if (!empresa.logo_objeto_id) return empresa;
    return {
      ...empresa,
      logo_url: `/api/v1/storage/public/objetos/${empresa.logo_objeto_id}`,
    };
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
