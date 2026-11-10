import { Injectable } from '@nestjs/common';
import {
  ConflictError,
  DomainError,
  ForbiddenError,
  NotFoundError,
  PgService,
  Rol,
  ROLES,
  esUuid,
  esViolacionUnicidad,
} from '@core/shared';

const CODIGO = /^[a-z][a-z0-9_]{1,39}$/;
const ROLES_ESCRITURA: Rol[] = [ROLES.EMPRESARIO, ROLES.VENDEDOR, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO];

export type TipoParte = 'persona' | 'organizacion';
export type EstadoParte = 'activo' | 'inactivo' | 'archivado';
export type TipoMedioContacto = 'email' | 'telefono' | 'whatsapp' | 'web' | 'otro';
export type TipoDireccion = 'fiscal' | 'comercial' | 'entrega' | 'operativa' | 'otro';

export interface ParteActor {
  user_id: string;
  rol: Rol;
  tenant_id?: string;
}

export interface Parte {
  id: string;
  empresa_id: string;
  tipo: TipoParte;
  nombre: string;
  nombre_comercial: string;
  identificador_tipo: string;
  identificador_valor: string;
  estado: EstadoParte;
  metadata: Record<string, unknown>;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
  eliminado_en: string | null;
}

export interface MedioContacto {
  id: string;
  empresa_id: string;
  parte_id: string;
  tipo: TipoMedioContacto;
  valor: string;
  etiqueta: string;
  es_principal: boolean;
  verificado_en: string | null;
  metadata: Record<string, unknown>;
  creado_en: string;
  actualizado_en: string;
  eliminado_en: string | null;
}

export interface DireccionParte {
  id: string;
  empresa_id: string;
  parte_id: string;
  tipo: TipoDireccion;
  etiqueta: string;
  linea_1: string;
  linea_2: string;
  ciudad: string;
  departamento: string;
  pais: string;
  codigo_postal: string;
  referencias: string;
  latitud: number | null;
  longitud: number | null;
  es_principal: boolean;
  metadata: Record<string, unknown>;
  creado_en: string;
  actualizado_en: string;
  eliminado_en: string | null;
}

export interface RelacionParte {
  id: string;
  empresa_id: string;
  parte_origen_id: string;
  parte_destino_id: string;
  tipo: string;
  metadata: Record<string, unknown>;
  activo: boolean;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
  parte_relacionada?: { id: string; nombre: string; tipo: TipoParte };
}

export interface CrearParteInput {
  tipo: TipoParte;
  nombre: string;
  nombre_comercial?: string;
  identificador_tipo?: string;
  identificador_valor?: string;
  estado?: EstadoParte;
  metadata?: Record<string, unknown>;
}

export interface ActualizarParteInput {
  tipo?: TipoParte;
  nombre?: string;
  nombre_comercial?: string;
  identificador_tipo?: string;
  identificador_valor?: string;
  estado?: EstadoParte;
  metadata?: Record<string, unknown>;
}

export interface CrearMedioContactoInput {
  tipo: TipoMedioContacto;
  valor: string;
  etiqueta?: string;
  es_principal?: boolean;
  metadata?: Record<string, unknown>;
}

export interface ActualizarMedioContactoInput {
  tipo?: TipoMedioContacto;
  valor?: string;
  etiqueta?: string;
  es_principal?: boolean;
  metadata?: Record<string, unknown>;
}

export interface CrearDireccionInput {
  tipo: TipoDireccion;
  etiqueta?: string;
  linea_1: string;
  linea_2?: string;
  ciudad?: string;
  departamento?: string;
  pais?: string;
  codigo_postal?: string;
  referencias?: string;
  latitud?: number | null;
  longitud?: number | null;
  es_principal?: boolean;
  metadata?: Record<string, unknown>;
}

export type ActualizarDireccionInput = Partial<CrearDireccionInput>;

interface ListarPartesOpciones {
  q?: string;
  tipo?: TipoParte;
  limite?: number;
}

@Injectable()
export class PartesService {
  constructor(private readonly pg: PgService) {}

  async listar(empresaId: string, opciones: ListarPartesOpciones, actor: ParteActor): Promise<Parte[]> {
    await this.validarAcceso(empresaId, actor, false);
    const condiciones = ['empresa_id = $1', 'eliminado_en IS NULL'];
    const valores: unknown[] = [empresaId];
    if (opciones.tipo) {
      condiciones.push(`tipo = $${valores.length + 1}`);
      valores.push(opciones.tipo);
    }
    if (opciones.q?.trim()) {
      condiciones.push(`(nombre ILIKE $${valores.length + 1} OR nombre_comercial ILIKE $${valores.length + 1} OR identificador_valor ILIKE $${valores.length + 1})`);
      valores.push(`%${opciones.q.trim()}%`);
    }
    const limite = Math.min(Math.max(opciones.limite ?? 50, 1), 100);
    valores.push(limite);
    return this.pg.query<Parte>(
      `SELECT id, empresa_id, tipo, nombre, nombre_comercial, identificador_tipo,
              identificador_valor, estado, metadata, creado_por, creado_en,
              actualizado_en, eliminado_en
         FROM identity.partes
        WHERE ${condiciones.join(' AND ')}
        ORDER BY actualizado_en DESC, id DESC
        LIMIT $${valores.length}`,
      valores,
    );
  }

  async obtener(empresaId: string, parteId: string, actor: ParteActor): Promise<Parte> {
    await this.validarAcceso(empresaId, actor, false);
    return this.obtenerParte(empresaId, parteId);
  }

  async crear(empresaId: string, input: CrearParteInput, actor: ParteActor): Promise<Parte> {
    await this.validarAcceso(empresaId, actor, true);
    const datos = this.normalizarParte(input);
    try {
      const fila = await this.pg.queryOne<Parte>(
        `INSERT INTO identity.partes
           (empresa_id, tipo, nombre, nombre_comercial, identificador_tipo,
            identificador_valor, estado, metadata, creado_por)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9)
         RETURNING id, empresa_id, tipo, nombre, nombre_comercial, identificador_tipo,
                   identificador_valor, estado, metadata, creado_por, creado_en,
                   actualizado_en, eliminado_en`,
        [empresaId, datos.tipo, datos.nombre, datos.nombre_comercial, datos.identificador_tipo,
          datos.identificador_valor, datos.estado, JSON.stringify(datos.metadata), actor.user_id],
      );
      if (!fila) throw new ConflictError('No se pudo crear la parte.');
      return fila;
    } catch (error) {
      if (esViolacionUnicidad(error)) throw new ConflictError('Ya existe una parte con ese identificador.');
      throw error;
    }
  }

  async actualizar(empresaId: string, parteId: string, input: ActualizarParteInput, actor: ParteActor): Promise<Parte> {
    await this.validarAcceso(empresaId, actor, true);
    this.validarUuid(parteId, 'PARTE_INVALIDA');
    const datos = this.normalizarParteParcial(input);
    try {
      const fila = await this.pg.queryOne<Parte>(
        `UPDATE identity.partes
            SET tipo = COALESCE($3, tipo),
                nombre = COALESCE($4, nombre),
                nombre_comercial = COALESCE($5, nombre_comercial),
                identificador_tipo = COALESCE($6, identificador_tipo),
                identificador_valor = COALESCE($7, identificador_valor),
                estado = COALESCE($8, estado),
                metadata = COALESCE($9::jsonb, metadata),
                actualizado_en = NOW()
          WHERE empresa_id = $1 AND id = $2 AND eliminado_en IS NULL
        RETURNING id, empresa_id, tipo, nombre, nombre_comercial, identificador_tipo,
                  identificador_valor, estado, metadata, creado_por, creado_en,
                  actualizado_en, eliminado_en`,
        [empresaId, parteId, datos.tipo, datos.nombre, datos.nombre_comercial,
          datos.identificador_tipo, datos.identificador_valor, datos.estado,
          datos.metadata === undefined ? null : JSON.stringify(datos.metadata)],
      );
      if (!fila) throw new NotFoundError('Parte', parteId);
      return fila;
    } catch (error) {
      if (esViolacionUnicidad(error)) throw new ConflictError('Ya existe una parte con ese identificador.');
      throw error;
    }
  }

  async eliminar(empresaId: string, parteId: string, actor: ParteActor): Promise<{ ok: true }> {
    await this.validarAcceso(empresaId, actor, true);
    this.validarUuid(parteId, 'PARTE_INVALIDA');
    await this.pg.transaccion(async (client) => {
      const resultado = await client.query(
        `UPDATE identity.partes
            SET estado = 'archivado', eliminado_en = NOW(), actualizado_en = NOW()
          WHERE empresa_id = $1 AND id = $2 AND eliminado_en IS NULL
        RETURNING id`,
        [empresaId, parteId],
      );
      if (!resultado.rowCount) throw new NotFoundError('Parte', parteId);
      await client.query(
        `UPDATE identity.medios_contacto SET eliminado_en = NOW(), actualizado_en = NOW()
          WHERE empresa_id = $1 AND parte_id = $2 AND eliminado_en IS NULL`,
        [empresaId, parteId],
      );
      await client.query(
        `UPDATE identity.direcciones_partes SET eliminado_en = NOW(), actualizado_en = NOW()
          WHERE empresa_id = $1 AND parte_id = $2 AND eliminado_en IS NULL`,
        [empresaId, parteId],
      );
      await client.query(
        `UPDATE identity.relaciones_partes SET activo = false, actualizado_en = NOW()
          WHERE empresa_id = $1 AND (parte_origen_id = $2 OR parte_destino_id = $2) AND activo = true`,
        [empresaId, parteId],
      );
    });
    return { ok: true };
  }

  /** Validación interna (servicio→servicio): ¿la parte vive en esa empresa? */
  async perteneceAEmpresa(empresaId: string, parteId: string): Promise<boolean> {
    if (!esUuid(empresaId) || !esUuid(parteId)) return false;
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM identity.partes
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [parteId, empresaId],
    );
    return !!fila;
  }

  async listarContactos(empresaId: string, parteId: string, actor: ParteActor): Promise<MedioContacto[]> {
    await this.validarParteAcceso(empresaId, parteId, actor, false);
    return this.pg.query<MedioContacto>(
      `SELECT id, empresa_id, parte_id, tipo, valor, etiqueta, es_principal,
              verificado_en, metadata, creado_en, actualizado_en, eliminado_en
         FROM identity.medios_contacto
        WHERE empresa_id = $1 AND parte_id = $2 AND eliminado_en IS NULL
        ORDER BY es_principal DESC, tipo, valor`,
      [empresaId, parteId],
    );
  }

  async crearContacto(empresaId: string, parteId: string, input: CrearMedioContactoInput, actor: ParteActor): Promise<MedioContacto> {
    await this.validarParteAcceso(empresaId, parteId, actor, true);
    const datos = this.normalizarContacto(input);
    try {
      return await this.guardarContacto(empresaId, parteId, datos);
    } catch (error) {
      if (esViolacionUnicidad(error)) throw new ConflictError('Ya existe ese medio de contacto para la parte.');
      throw error;
    }
  }

  async actualizarContacto(empresaId: string, parteId: string, contactoId: string, input: ActualizarMedioContactoInput, actor: ParteActor): Promise<MedioContacto> {
    await this.validarParteAcceso(empresaId, parteId, actor, true);
    this.validarUuid(contactoId, 'CONTACTO_INVALIDO');
    const datos = this.normalizarContactoParcial(input);
    try {
      const fila = await this.pg.queryOne<MedioContacto>(
        `UPDATE identity.medios_contacto
            SET tipo = COALESCE($4, tipo), valor = COALESCE($5, valor),
                etiqueta = COALESCE($6, etiqueta), es_principal = COALESCE($7, es_principal),
                metadata = COALESCE($8::jsonb, metadata), actualizado_en = NOW()
          WHERE empresa_id = $1 AND parte_id = $2 AND id = $3 AND eliminado_en IS NULL
        RETURNING id, empresa_id, parte_id, tipo, valor, etiqueta, es_principal,
                  verificado_en, metadata, creado_en, actualizado_en, eliminado_en`,
        [empresaId, parteId, contactoId, datos.tipo, datos.valor, datos.etiqueta,
          datos.es_principal, datos.metadata === undefined ? null : JSON.stringify(datos.metadata)],
      );
      if (!fila) throw new NotFoundError('Medio de contacto', contactoId);
      return fila;
    } catch (error) {
      if (esViolacionUnicidad(error)) throw new ConflictError('Ya existe ese medio de contacto para la parte.');
      throw error;
    }
  }

  async eliminarContacto(empresaId: string, parteId: string, contactoId: string, actor: ParteActor): Promise<{ ok: true }> {
    await this.validarParteAcceso(empresaId, parteId, actor, true);
    this.validarUuid(contactoId, 'CONTACTO_INVALIDO');
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE identity.medios_contacto SET eliminado_en = NOW(), actualizado_en = NOW()
        WHERE empresa_id = $1 AND parte_id = $2 AND id = $3 AND eliminado_en IS NULL
      RETURNING id`,
      [empresaId, parteId, contactoId],
    );
    if (!fila) throw new NotFoundError('Medio de contacto', contactoId);
    return { ok: true };
  }

  async listarDirecciones(empresaId: string, parteId: string, actor: ParteActor): Promise<DireccionParte[]> {
    await this.validarParteAcceso(empresaId, parteId, actor, false);
    return this.pg.query<DireccionParte>(
      `SELECT id, empresa_id, parte_id, tipo, etiqueta, linea_1, linea_2,
              ciudad, departamento, pais, codigo_postal, referencias, latitud,
              longitud, es_principal, metadata, creado_en, actualizado_en, eliminado_en
         FROM identity.direcciones_partes
        WHERE empresa_id = $1 AND parte_id = $2 AND eliminado_en IS NULL
        ORDER BY es_principal DESC, tipo, id`,
      [empresaId, parteId],
    );
  }

  async crearDireccion(empresaId: string, parteId: string, input: CrearDireccionInput, actor: ParteActor): Promise<DireccionParte> {
    await this.validarParteAcceso(empresaId, parteId, actor, true);
    const datos = this.normalizarDireccion(input);
    return this.guardarDireccion(empresaId, parteId, datos);
  }

  async actualizarDireccion(empresaId: string, parteId: string, direccionId: string, input: ActualizarDireccionInput, actor: ParteActor): Promise<DireccionParte> {
    await this.validarParteAcceso(empresaId, parteId, actor, true);
    this.validarUuid(direccionId, 'DIRECCION_INVALIDA');
    const datos = this.normalizarDireccionParcial(input);
    const fila = await this.pg.queryOne<DireccionParte>(
      `UPDATE identity.direcciones_partes
          SET tipo = COALESCE($4, tipo), etiqueta = COALESCE($5, etiqueta),
              linea_1 = COALESCE($6, linea_1), linea_2 = COALESCE($7, linea_2),
              ciudad = COALESCE($8, ciudad), departamento = COALESCE($9, departamento),
              pais = COALESCE($10, pais), codigo_postal = COALESCE($11, codigo_postal),
              referencias = COALESCE($12, referencias), latitud = COALESCE($13, latitud),
              longitud = COALESCE($14, longitud), es_principal = COALESCE($15, es_principal),
              metadata = COALESCE($16::jsonb, metadata), actualizado_en = NOW()
        WHERE empresa_id = $1 AND parte_id = $2 AND id = $3 AND eliminado_en IS NULL
      RETURNING id, empresa_id, parte_id, tipo, etiqueta, linea_1, linea_2,
                ciudad, departamento, pais, codigo_postal, referencias, latitud,
                longitud, es_principal, metadata, creado_en, actualizado_en, eliminado_en`,
      [empresaId, parteId, direccionId, datos.tipo, datos.etiqueta, datos.linea_1,
        datos.linea_2, datos.ciudad, datos.departamento, datos.pais, datos.codigo_postal,
        datos.referencias, datos.latitud, datos.longitud, datos.es_principal,
        datos.metadata === undefined ? null : JSON.stringify(datos.metadata)],
    );
    if (!fila) throw new NotFoundError('Dirección', direccionId);
    return fila;
  }

  async eliminarDireccion(empresaId: string, parteId: string, direccionId: string, actor: ParteActor): Promise<{ ok: true }> {
    await this.validarParteAcceso(empresaId, parteId, actor, true);
    this.validarUuid(direccionId, 'DIRECCION_INVALIDA');
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE identity.direcciones_partes SET eliminado_en = NOW(), actualizado_en = NOW()
        WHERE empresa_id = $1 AND parte_id = $2 AND id = $3 AND eliminado_en IS NULL
      RETURNING id`,
      [empresaId, parteId, direccionId],
    );
    if (!fila) throw new NotFoundError('Dirección', direccionId);
    return { ok: true };
  }

  async listarRelaciones(empresaId: string, parteId: string, actor: ParteActor): Promise<RelacionParte[]> {
    await this.validarParteAcceso(empresaId, parteId, actor, false);
    return this.pg.query<RelacionParte>(
      `SELECT r.id, r.empresa_id, r.parte_origen_id, r.parte_destino_id,
              r.tipo, r.metadata, r.activo, r.creado_por, r.creado_en,
              r.actualizado_en, p.id AS relacionada_id, p.nombre AS relacionada_nombre,
              p.tipo AS relacionada_tipo
         FROM identity.relaciones_partes r
         JOIN identity.partes p
           ON p.id = CASE WHEN r.parte_origen_id = $2 THEN r.parte_destino_id ELSE r.parte_origen_id END
        WHERE r.empresa_id = $1 AND (r.parte_origen_id = $2 OR r.parte_destino_id = $2)
          AND r.activo = true AND p.eliminado_en IS NULL
        ORDER BY r.tipo, r.creado_en DESC`,
      [empresaId, parteId],
    ).then((filas) => filas.map((fila) => ({
      ...fila,
      parte_relacionada: {
        id: (fila as unknown as { relacionada_id: string }).relacionada_id,
        nombre: (fila as unknown as { relacionada_nombre: string }).relacionada_nombre,
        tipo: (fila as unknown as { relacionada_tipo: TipoParte }).relacionada_tipo,
      },
    })));
  }

  async crearRelacion(empresaId: string, parteOrigenId: string, parteDestinoId: string, tipo: string, metadata: Record<string, unknown> | undefined, actor: ParteActor): Promise<RelacionParte> {
    await this.validarParteAcceso(empresaId, parteOrigenId, actor, true);
    this.validarUuid(parteDestinoId, 'PARTE_DESTINO_INVALIDA');
    const tipoNormalizado = this.normalizarCodigo(tipo, 'TIPO_RELACION_INVALIDO');
    this.validarMetadata(metadata);
    if (parteOrigenId === parteDestinoId) throw new DomainError('RELACION_INVALIDA', 'Una parte no puede relacionarse consigo misma.');
    await this.obtenerParte(empresaId, parteDestinoId);
    try {
      const fila = await this.pg.queryOne<RelacionParte>(
        `INSERT INTO identity.relaciones_partes
           (empresa_id, parte_origen_id, parte_destino_id, tipo, metadata, creado_por)
         VALUES ($1, $2, $3, $4, $5::jsonb, $6)
         RETURNING id, empresa_id, parte_origen_id, parte_destino_id, tipo,
                   metadata, activo, creado_por, creado_en, actualizado_en`,
        [empresaId, parteOrigenId, parteDestinoId, tipoNormalizado, JSON.stringify(metadata ?? {}), actor.user_id],
      );
      if (!fila) throw new ConflictError('No se pudo crear la relación.');
      return fila;
    } catch (error) {
      if (esViolacionUnicidad(error)) throw new ConflictError('La relación ya existe.');
      throw error;
    }
  }

  async eliminarRelacion(empresaId: string, parteId: string, relacionId: string, actor: ParteActor): Promise<{ ok: true }> {
    await this.validarParteAcceso(empresaId, parteId, actor, true);
    this.validarUuid(relacionId, 'RELACION_INVALIDA');
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE identity.relaciones_partes SET activo = false, actualizado_en = NOW()
        WHERE empresa_id = $1 AND id = $2
          AND (parte_origen_id = $3 OR parte_destino_id = $3) AND activo = true
      RETURNING id`,
      [empresaId, relacionId, parteId],
    );
    if (!fila) throw new NotFoundError('Relación', relacionId);
    return { ok: true };
  }

  private async guardarContacto(empresaId: string, parteId: string, datos: CrearMedioContactoInput): Promise<MedioContacto> {
    const fila = await this.pg.queryOne<MedioContacto>(
      `INSERT INTO identity.medios_contacto
         (empresa_id, parte_id, tipo, valor, etiqueta, es_principal, metadata)
       VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb)
       RETURNING id, empresa_id, parte_id, tipo, valor, etiqueta, es_principal,
                 verificado_en, metadata, creado_en, actualizado_en, eliminado_en`,
      [empresaId, parteId, datos.tipo, datos.valor, datos.etiqueta ?? '', datos.es_principal ?? false, JSON.stringify(datos.metadata ?? {})],
    );
    if (!fila) throw new ConflictError('No se pudo crear el medio de contacto.');
    return fila;
  }

  private async guardarDireccion(empresaId: string, parteId: string, datos: CrearDireccionInput): Promise<DireccionParte> {
    const fila = await this.pg.queryOne<DireccionParte>(
      `INSERT INTO identity.direcciones_partes
         (empresa_id, parte_id, tipo, etiqueta, linea_1, linea_2, ciudad,
          departamento, pais, codigo_postal, referencias, latitud, longitud,
          es_principal, metadata)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15::jsonb)
       RETURNING id, empresa_id, parte_id, tipo, etiqueta, linea_1, linea_2,
                 ciudad, departamento, pais, codigo_postal, referencias, latitud,
                 longitud, es_principal, metadata, creado_en, actualizado_en, eliminado_en`,
      [empresaId, parteId, datos.tipo, datos.etiqueta ?? '', datos.linea_1, datos.linea_2 ?? '',
        datos.ciudad ?? '', datos.departamento ?? '', datos.pais ?? '', datos.codigo_postal ?? '',
        datos.referencias ?? '', datos.latitud ?? null, datos.longitud ?? null,
        datos.es_principal ?? false, JSON.stringify(datos.metadata ?? {})],
    );
    if (!fila) throw new ConflictError('No se pudo crear la dirección.');
    return fila;
  }

  private async obtenerParte(empresaId: string, parteId: string): Promise<Parte> {
    this.validarUuid(parteId, 'PARTE_INVALIDA');
    const fila = await this.pg.queryOne<Parte>(
      `SELECT id, empresa_id, tipo, nombre, nombre_comercial, identificador_tipo,
              identificador_valor, estado, metadata, creado_por, creado_en,
              actualizado_en, eliminado_en
         FROM identity.partes
        WHERE empresa_id = $1 AND id = $2 AND eliminado_en IS NULL`,
      [empresaId, parteId],
    );
    if (!fila) throw new NotFoundError('Parte', parteId);
    return fila;
  }

  private normalizarParte(input: CrearParteInput): Required<CrearParteInput> {
    const datos = this.normalizarParteParcial(input);
    if (!datos.tipo || !datos.nombre) throw new DomainError('PARTE_INVALIDA', 'El tipo y nombre de la parte son obligatorios.');
    return {
      tipo: datos.tipo,
      nombre: datos.nombre,
      nombre_comercial: datos.nombre_comercial ?? '',
      identificador_tipo: datos.identificador_tipo ?? '',
      identificador_valor: datos.identificador_valor ?? '',
      estado: datos.estado ?? 'activo',
      metadata: datos.metadata ?? {},
    };
  }

  private normalizarParteParcial(input: ActualizarParteInput): Partial<CrearParteInput> {
    if (input.tipo !== undefined && !['persona', 'organizacion'].includes(input.tipo)) throw new DomainError('TIPO_PARTE_INVALIDO', 'El tipo de parte no es válido.');
    if (input.estado !== undefined && !['activo', 'inactivo', 'archivado'].includes(input.estado)) throw new DomainError('ESTADO_PARTE_INVALIDO', 'El estado de parte no es válido.');
    const nombre = input.nombre === undefined ? undefined : String(input.nombre).trim();
    if (nombre !== undefined && (nombre.length < 1 || nombre.length > 240)) throw new DomainError('NOMBRE_PARTE_INVALIDO', 'El nombre debe tener entre 1 y 240 caracteres.');
    const metadata = input.metadata === undefined ? undefined : this.validarMetadata(input.metadata);
    return {
      ...(input.tipo === undefined ? {} : { tipo: input.tipo }),
      ...(nombre === undefined ? {} : { nombre }),
      ...(input.nombre_comercial === undefined ? {} : { nombre_comercial: this.texto(input.nombre_comercial, 240) }),
      ...(input.identificador_tipo === undefined ? {} : { identificador_tipo: this.texto(input.identificador_tipo, 40) }),
      ...(input.identificador_valor === undefined ? {} : { identificador_valor: this.texto(input.identificador_valor, 120) }),
      ...(input.estado === undefined ? {} : { estado: input.estado }),
      ...(metadata === undefined ? {} : { metadata }),
    };
  }

  private normalizarContacto(input: CrearMedioContactoInput): Required<CrearMedioContactoInput> {
    const datos = this.normalizarContactoParcial(input);
    if (!datos.tipo || !datos.valor) throw new DomainError('CONTACTO_INVALIDO', 'El tipo y valor del contacto son obligatorios.');
    return { tipo: datos.tipo, valor: datos.valor, etiqueta: datos.etiqueta ?? '', es_principal: datos.es_principal ?? false, metadata: datos.metadata ?? {} };
  }

  private normalizarContactoParcial(input: ActualizarMedioContactoInput): Partial<CrearMedioContactoInput> {
    if (input.tipo !== undefined && !['email', 'telefono', 'whatsapp', 'web', 'otro'].includes(input.tipo)) throw new DomainError('TIPO_CONTACTO_INVALIDO', 'El tipo de contacto no es válido.');
    const valor = input.valor === undefined ? undefined : this.texto(input.valor, 240);
    return {
      ...(input.tipo === undefined ? {} : { tipo: input.tipo }),
      ...(valor === undefined ? {} : { valor }),
      ...(input.etiqueta === undefined ? {} : { etiqueta: this.texto(input.etiqueta, 120) }),
      ...(input.es_principal === undefined ? {} : { es_principal: input.es_principal }),
      ...(input.metadata === undefined ? {} : { metadata: this.validarMetadata(input.metadata) }),
    };
  }

  private normalizarDireccion(input: CrearDireccionInput): Required<CrearDireccionInput> {
    const datos = this.normalizarDireccionParcial(input);
    if (!datos.tipo || !datos.linea_1) throw new DomainError('DIRECCION_INVALIDA', 'El tipo y la línea principal son obligatorios.');
    return {
      tipo: datos.tipo,
      etiqueta: datos.etiqueta ?? '',
      linea_1: datos.linea_1,
      linea_2: datos.linea_2 ?? '',
      ciudad: datos.ciudad ?? '',
      departamento: datos.departamento ?? '',
      pais: datos.pais ?? '',
      codigo_postal: datos.codigo_postal ?? '',
      referencias: datos.referencias ?? '',
      latitud: datos.latitud ?? null,
      longitud: datos.longitud ?? null,
      es_principal: datos.es_principal ?? false,
      metadata: datos.metadata ?? {},
    };
  }

  private normalizarDireccionParcial(input: ActualizarDireccionInput): Partial<CrearDireccionInput> {
    if (input.tipo !== undefined && !['fiscal', 'comercial', 'entrega', 'operativa', 'otro'].includes(input.tipo)) throw new DomainError('TIPO_DIRECCION_INVALIDO', 'El tipo de dirección no es válido.');
    const texto = (valor: string | undefined, max: number) => valor === undefined ? undefined : this.texto(valor, max);
    if (input.latitud !== undefined && input.latitud !== null && (input.latitud < -90 || input.latitud > 90)) throw new DomainError('COORDENADA_INVALIDA', 'La latitud debe estar entre -90 y 90.');
    if (input.longitud !== undefined && input.longitud !== null && (input.longitud < -180 || input.longitud > 180)) throw new DomainError('COORDENADA_INVALIDA', 'La longitud debe estar entre -180 y 180.');
    return {
      ...(input.tipo === undefined ? {} : { tipo: input.tipo }),
      ...(input.etiqueta === undefined ? {} : { etiqueta: texto(input.etiqueta, 120) }),
      ...(input.linea_1 === undefined ? {} : { linea_1: texto(input.linea_1, 240) }),
      ...(input.linea_2 === undefined ? {} : { linea_2: texto(input.linea_2, 240) }),
      ...(input.ciudad === undefined ? {} : { ciudad: texto(input.ciudad, 120) }),
      ...(input.departamento === undefined ? {} : { departamento: texto(input.departamento, 120) }),
      ...(input.pais === undefined ? {} : { pais: texto(input.pais, 120) }),
      ...(input.codigo_postal === undefined ? {} : { codigo_postal: texto(input.codigo_postal, 32) }),
      ...(input.referencias === undefined ? {} : { referencias: texto(input.referencias, 500) }),
      ...(input.latitud === undefined ? {} : { latitud: input.latitud }),
      ...(input.longitud === undefined ? {} : { longitud: input.longitud }),
      ...(input.es_principal === undefined ? {} : { es_principal: input.es_principal }),
      ...(input.metadata === undefined ? {} : { metadata: this.validarMetadata(input.metadata) }),
    };
  }

  private texto(valor: string, max: number): string {
    const resultado = String(valor).trim();
    if (resultado.length > max) throw new DomainError('VALOR_INVALIDO', `El valor no puede superar ${max} caracteres.`);
    return resultado;
  }

  private validarMetadata(metadata: Record<string, unknown> | undefined): Record<string, unknown> | undefined {
    if (metadata === undefined) return undefined;
    if (!metadata || typeof metadata !== 'object' || Array.isArray(metadata) || JSON.stringify(metadata).length > 64 * 1024) throw new DomainError('METADATA_INVALIDA', 'La metadata debe ser un objeto JSON de hasta 64 KiB.');
    return metadata;
  }

  private normalizarCodigo(valor: string, codigo: string): string {
    const resultado = String(valor ?? '').trim().toLowerCase();
    if (!CODIGO.test(resultado)) throw new DomainError(codigo, 'El código solo admite minúsculas, números y guion bajo (2-40).');
    return resultado;
  }

  private validarUuid(valor: string, codigo: string): void {
    if (!esUuid(valor)) throw new DomainError(codigo, 'El identificador debe ser un UUID válido.');
  }

  private async validarParteAcceso(empresaId: string, parteId: string, actor: ParteActor, escritura: boolean): Promise<void> {
    await this.validarAcceso(empresaId, actor, escritura);
    await this.obtenerParte(empresaId, parteId);
  }

  private async validarAcceso(empresaId: string, actor: ParteActor, escritura: boolean): Promise<void> {
    this.validarUuid(empresaId, 'EMPRESA_INVALIDA');
    const global = actor.rol === ROLES.SUPER_ADMIN || actor.rol === ROLES.ADMIN;
    if (global) return;
    if (!actor.tenant_id || actor.tenant_id !== empresaId) throw new ForbiddenError('No puedes operar partes de otra empresa.');
    if (escritura && !ROLES_ESCRITURA.includes(actor.rol)) throw new ForbiddenError('El rol no puede modificar los datos universales.');
    const miembro = await this.pg.queryOne(
      `SELECT 1 FROM identity.asignaciones_personal
        WHERE empresa_id = $1 AND usuario_id = $2 AND estado = 'activa'`,
      [empresaId, actor.user_id],
    );
    if (!miembro) throw new ForbiddenError('El usuario no es miembro activo de esta empresa.');
  }
}
