import { Injectable } from '@nestjs/common';
import Ajv, { type ErrorObject } from 'ajv';
import {
  ConflictError,
  DomainError,
  ForbiddenError,
  NotFoundError,
  PgService,
  Rol,
  ROLES,
  esUuid,
} from '@core/shared';

const CODIGO = /^[a-z][a-z0-9_]{1,39}$/;
const MAX_JSON_BYTES = 64 * 1024;

export interface ExtensionActor {
  user_id: string;
  rol: Rol;
  tenant_id?: string;
}

export interface EmpresaExtension {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  recurso: string;
  descripcion: string;
  esquema: Record<string, unknown>;
  formulario: Record<string, unknown>;
  reglas: Record<string, unknown>;
  version: number;
  activo: boolean;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
}

export interface CrearExtensionInput {
  codigo: string;
  nombre: string;
  recurso: string;
  descripcion?: string;
  esquema: Record<string, unknown>;
  formulario?: Record<string, unknown>;
  reglas?: Record<string, unknown>;
  activo?: boolean;
}

export interface ExtensionValue {
  id: string;
  empresa_id: string;
  extension_id: string;
  recurso_id: string;
  valores: Record<string, unknown>;
  version_schema: number;
  creado_por: string;
  creado_en: string;
  actualizado_en: string;
}

type ExtensionRow = EmpresaExtension;
type ExtensionValueRow = ExtensionValue;
interface ActiveExtension {
  id: string;
  esquema: Record<string, unknown>;
  version: number;
}

@Injectable()
export class ExtensionesService {
  private readonly ajv = new Ajv({ allErrors: true, strict: false });

  constructor(private readonly pg: PgService) {}

  async listar(empresaId: string, actor: ExtensionActor): Promise<EmpresaExtension[]> {
    await this.validarAcceso(empresaId, actor, false);
    return this.pg.query<ExtensionRow>(
      `SELECT id, empresa_id, codigo, nombre, recurso, descripcion, esquema,
              formulario, reglas, version, activo, creado_por, creado_en, actualizado_en
         FROM identity.extensiones_empresa
        WHERE empresa_id = $1
        ORDER BY recurso, codigo, version DESC`,
      [empresaId],
    );
  }

  async crear(
    empresaId: string,
    datos: CrearExtensionInput,
    actor: ExtensionActor,
  ): Promise<EmpresaExtension> {
    await this.validarAcceso(empresaId, actor, true);
    const input = this.normalizar(datos);
    return this.pg.transaccion(async (client) => {
      await client.query(
        `SELECT pg_advisory_xact_lock(hashtext($1 || ':' || $2))`,
        [empresaId, input.codigo],
      );
      const version = await client.query<{ version: number }>(
        `SELECT COALESCE(MAX(version), 0) + 1 AS version
           FROM identity.extensiones_empresa
          WHERE empresa_id = $1 AND codigo = $2`,
        [empresaId, input.codigo],
      );
      const siguiente = version.rows[0]?.version ?? 1;
      if (input.activo) {
        await client.query(
          `UPDATE identity.extensiones_empresa
              SET activo = false, actualizado_en = NOW()
            WHERE empresa_id = $1 AND codigo = $2 AND activo = true`,
          [empresaId, input.codigo],
        );
      }
      const resultado = await client.query<ExtensionRow>(
        `INSERT INTO identity.extensiones_empresa
           (empresa_id, codigo, nombre, recurso, descripcion, esquema, formulario,
            reglas, version, activo, creado_por)
         VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb, $9, $10, $11)
         RETURNING id, empresa_id, codigo, nombre, recurso, descripcion, esquema,
                   formulario, reglas, version, activo, creado_por, creado_en, actualizado_en`,
        [
          empresaId,
          input.codigo,
          input.nombre,
          input.recurso,
          input.descripcion,
          JSON.stringify(input.esquema),
          JSON.stringify(input.formulario),
          JSON.stringify(input.reglas),
          siguiente,
          input.activo,
          actor.user_id,
        ],
      );
      const fila = resultado.rows[0];
      if (!fila) throw new ConflictError('No se pudo crear la extensión.');
      return fila;
    });
  }

  async cambiarEstado(
    empresaId: string,
    codigo: string,
    activo: boolean,
    actor: ExtensionActor,
  ): Promise<EmpresaExtension> {
    await this.validarAcceso(empresaId, actor, true);
    const nombreCodigo = this.normalizarCodigo(codigo);
    const fila = await this.pg.queryOne<ExtensionRow>(
      `UPDATE identity.extensiones_empresa
          SET activo = $3, actualizado_en = NOW()
        WHERE empresa_id = $1 AND codigo = $2
          AND version = (
            SELECT MAX(version) FROM identity.extensiones_empresa
             WHERE empresa_id = $1 AND codigo = $2
          )
      RETURNING id, empresa_id, codigo, nombre, recurso, descripcion, esquema,
                formulario, reglas, version, activo, creado_por, creado_en, actualizado_en`,
      [empresaId, nombreCodigo, activo],
    );
    if (!fila) throw new NotFoundError('Extensión', nombreCodigo);
    return fila;
  }

  async validar(
    empresaId: string,
    codigo: string,
    datos: Record<string, unknown>,
    actor: ExtensionActor,
  ): Promise<{ valido: boolean; errores: Array<Record<string, unknown>> }> {
    await this.validarAcceso(empresaId, actor, false);
    const nombreCodigo = this.normalizarCodigo(codigo);
    const extension = await this.pg.queryOne<Pick<EmpresaExtension, 'esquema'>>(
      `SELECT esquema
         FROM identity.extensiones_empresa
        WHERE empresa_id = $1 AND codigo = $2 AND activo = true
        ORDER BY version DESC
        LIMIT 1`,
      [empresaId, nombreCodigo],
    );
    if (!extension) throw new NotFoundError('Extensión', nombreCodigo);
    if (!datos || typeof datos !== 'object' || Array.isArray(datos)) {
      return { valido: false, errores: [{ instancia: '', mensaje: 'Los datos deben ser un objeto JSON.' }] };
    }
    const valido = this.ajv.validate(extension.esquema, datos);
    return {
      valido: Boolean(valido),
      errores: valido ? [] : this.formatearErrores(this.ajv.errors),
    };
  }

  async guardarValores(
    empresaId: string,
    codigo: string,
    recursoId: string,
    valores: Record<string, unknown>,
    actor: ExtensionActor,
  ): Promise<ExtensionValue> {
    await this.validarAcceso(empresaId, actor, true);
    const recursoUuid = this.validarRecursoId(recursoId);
    const extension = await this.obtenerExtensionActiva(empresaId, codigo);
    this.validarValores(extension.esquema, valores);
    const fila = await this.pg.queryOne<ExtensionValueRow>(
      `INSERT INTO identity.extensiones_empresa_valores
         (empresa_id, extension_id, recurso_id, valores, version_schema, creado_por)
       VALUES ($1, $2, $3, $4::jsonb, $5, $6)
       ON CONFLICT (empresa_id, extension_id, recurso_id) DO UPDATE
         SET valores = EXCLUDED.valores,
             version_schema = EXCLUDED.version_schema,
             actualizado_en = NOW()
       RETURNING id, empresa_id, extension_id, recurso_id, valores,
                 version_schema, creado_por, creado_en, actualizado_en`,
      [empresaId, extension.id, recursoUuid, JSON.stringify(valores), extension.version, actor.user_id],
    );
    if (!fila) throw new ConflictError('No se pudieron guardar los valores de la extensión.');
    return fila;
  }

  async obtenerValores(
    empresaId: string,
    codigo: string,
    recursoId: string,
    actor: ExtensionActor,
  ): Promise<ExtensionValue | null> {
    await this.validarAcceso(empresaId, actor, false);
    const recursoUuid = this.validarRecursoId(recursoId);
    const extension = await this.obtenerExtensionActiva(empresaId, codigo);
    return this.pg.queryOne<ExtensionValueRow>(
      `SELECT id, empresa_id, extension_id, recurso_id, valores,
              version_schema, creado_por, creado_en, actualizado_en
         FROM identity.extensiones_empresa_valores
        WHERE empresa_id = $1 AND extension_id = $2 AND recurso_id = $3`,
      [empresaId, extension.id, recursoUuid],
    );
  }

  async eliminarValores(
    empresaId: string,
    codigo: string,
    recursoId: string,
    actor: ExtensionActor,
  ): Promise<void> {
    await this.validarAcceso(empresaId, actor, true);
    const recursoUuid = this.validarRecursoId(recursoId);
    const extension = await this.obtenerExtensionActiva(empresaId, codigo);
    const eliminado = await this.pg.queryOne<{ id: string }>(
      `DELETE FROM identity.extensiones_empresa_valores
        WHERE empresa_id = $1 AND extension_id = $2 AND recurso_id = $3
      RETURNING id`,
      [empresaId, extension.id, recursoUuid],
    );
    if (!eliminado) throw new NotFoundError('Valores de extensión', recursoUuid);
  }

  private normalizar(datos: CrearExtensionInput): Required<CrearExtensionInput> {
    const codigo = this.normalizarCodigo(datos.codigo);
    const recurso = this.normalizarCodigo(datos.recurso);
    const nombre = String(datos.nombre ?? '').trim();
    if (nombre.length < 2 || nombre.length > 120) {
      throw new DomainError('NOMBRE_EXTENSION_INVALIDO', 'El nombre debe tener entre 2 y 120 caracteres.');
    }
    const descripcion = String(datos.descripcion ?? '').trim();
    if (descripcion.length > 1000) {
      throw new DomainError('DESCRIPCION_EXTENSION_INVALIDA', 'La descripción no puede superar 1000 caracteres.');
    }
    const esquema = this.validarObjeto(datos.esquema, 'ESQUEMA_EXTENSION_INVALIDO');
    try {
      this.ajv.compile(esquema);
    } catch (error) {
      throw new DomainError('ESQUEMA_EXTENSION_INVALIDO', 'El esquema JSON no es válido.', {
        mensaje: error instanceof Error ? error.message : 'schema inválido',
      });
    }
    const formulario = this.validarObjeto(datos.formulario ?? {}, 'FORMULARIO_EXTENSION_INVALIDO');
    const reglas = this.validarObjeto(datos.reglas ?? {}, 'REGLAS_EXTENSION_INVALIDAS');
    return { codigo, nombre, recurso, descripcion, esquema, formulario, reglas, activo: datos.activo !== false };
  }

  private validarObjeto(valor: unknown, codigo: string): Record<string, unknown> {
    if (!valor || typeof valor !== 'object' || Array.isArray(valor)) {
      throw new DomainError(codigo, 'El valor debe ser un objeto JSON.');
    }
    if (JSON.stringify(valor).length > MAX_JSON_BYTES) {
      throw new DomainError(codigo, `El objeto no puede superar ${MAX_JSON_BYTES} bytes.`);
    }
    if (this.contieneRef(valor)) {
      throw new DomainError(codigo, 'Las referencias externas ($ref) no están permitidas.');
    }
    return valor as Record<string, unknown>;
  }

  private contieneRef(valor: unknown): boolean {
    if (Array.isArray(valor)) return valor.some((item) => this.contieneRef(item));
    if (!valor || typeof valor !== 'object') return false;
    return Object.entries(valor).some(([clave, item]) => clave === '$ref' || this.contieneRef(item));
  }

  private formatearErrores(errores: ErrorObject[] | null | undefined): Array<Record<string, unknown>> {
    return (errores ?? []).map((error) => ({
      instancia: error.instancePath,
      parametro: error.params,
      mensaje: error.message ?? 'dato inválido',
    }));
  }

  private validarValores(esquema: Record<string, unknown>, valores: Record<string, unknown>): void {
    if (!valores || typeof valores !== 'object' || Array.isArray(valores)) {
      throw new DomainError('VALORES_EXTENSION_INVALIDOS', 'Los valores deben ser un objeto JSON.');
    }
    if (JSON.stringify(valores).length > MAX_JSON_BYTES) {
      throw new DomainError('VALORES_EXTENSION_INVALIDOS', `Los valores no pueden superar ${MAX_JSON_BYTES} bytes.`);
    }
    const valido = this.ajv.validate(esquema, valores);
    if (!valido) {
      throw new DomainError(
        'VALORES_EXTENSION_INVALIDOS',
        'Los valores no cumplen el JSON Schema de la extensión.',
        { errores: this.formatearErrores(this.ajv.errors) },
      );
    }
  }

  private async obtenerExtensionActiva(empresaId: string, codigo: string): Promise<ActiveExtension> {
    const nombreCodigo = this.normalizarCodigo(codigo);
    const extension = await this.pg.queryOne<ActiveExtension>(
      `SELECT id, esquema, version
         FROM identity.extensiones_empresa
        WHERE empresa_id = $1 AND codigo = $2 AND activo = true
        ORDER BY version DESC
        LIMIT 1`,
      [empresaId, nombreCodigo],
    );
    if (!extension) throw new NotFoundError('Extensión', nombreCodigo);
    return extension;
  }

  private validarRecursoId(valor: string): string {
    const recursoId = String(valor ?? '').trim();
    if (!esUuid(recursoId)) throw new DomainError('RECURSO_INVALIDO', 'El recurso debe ser un UUID válido.');
    return recursoId;
  }

  private normalizarCodigo(valor: unknown): string {
    const codigo = String(valor ?? '').trim().toLowerCase();
    if (!CODIGO.test(codigo)) {
      throw new DomainError('CODIGO_EXTENSION_INVALIDO', 'El código solo admite minúsculas, números y guion bajo (2-40).');
    }
    return codigo;
  }

  private async validarAcceso(empresaId: string, actor: ExtensionActor, escritura: boolean): Promise<void> {
    if (!esUuid(empresaId)) throw new DomainError('EMPRESA_INVALIDA', 'La empresa debe ser un UUID válido.');
    const global = actor.rol === ROLES.SUPER_ADMIN || actor.rol === ROLES.ADMIN;
    if (global) return;
    if (!actor.tenant_id || actor.tenant_id !== empresaId) {
      throw new ForbiddenError('No puedes operar extensiones de otra empresa.');
    }
    if (escritura && actor.rol !== ROLES.EMPRESARIO) {
      throw new ForbiddenError('Solo el empresario puede modificar extensiones.');
    }
    const miembro = await this.pg.queryOne(
      `SELECT 1 FROM identity.asignaciones_personal
        WHERE empresa_id = $1 AND usuario_id = $2 AND estado = 'activa'`,
      [empresaId, actor.user_id],
    );
    if (!miembro) throw new ForbiddenError('El usuario no es miembro activo de esta empresa.');
  }
}
