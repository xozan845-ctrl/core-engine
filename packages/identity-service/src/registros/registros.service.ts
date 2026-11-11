import { Injectable } from '@nestjs/common';
import {
  CAPACIDADES,
  CapabilityDisabledError,
  DomainError,
  ForbiddenError,
  NotFoundError,
  PgService,
  ROLES,
  Rol,
} from '@core/shared';
import { CapacidadesService } from '../capacidades/capacidades.service';

/**
 * Registros genéricos por parte (Fase A, ADR-0001/R-AR-5).
 *
 * Una sola primitiva cubre historia clínica, recetas, expedientes e
 * inspecciones: `tipo` + `datos` JSON + adjuntos y vínculo opcional a cita o
 * item. La capacidad decide el acceso (`recetas` o `historia_clinica`), nunca
 * el rubro.
 */

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const TIPO = /^[a-z][a-z0-9_]{1,39}$/;
const MAX_DATOS_BYTES = 64 * 1024;
const MAX_ADJUNTOS = 10;

const ROLES_ACCESO: Rol[] = [
  ROLES.SUPER_ADMIN,
  ROLES.ADMIN,
  ROLES.EMPRESARIO,
  ROLES.VENDEDOR,
  ROLES.COORDINADOR,
  ROLES.SUPERVISOR,
  ROLES.OPERATIVO,
];

export interface RegistroParte {
  id: string;
  empresa_id: string;
  parte_id: string;
  tipo: string;
  titulo: string;
  datos: Record<string, unknown>;
  adjuntos: string[];
  cita_id: string | null;
  item_id: string | null;
  autor_id: string;
  creado_en: string;
  actualizado_en: string;
}

export interface CrearRegistroInput {
  tipo: string;
  titulo?: string;
  datos?: Record<string, unknown>;
  adjuntos?: string[];
  cita_id?: string | null;
  item_id?: string | null;
}

export type ActualizarRegistroInput = Partial<CrearRegistroInput>;

interface RegistroFila {
  id: string;
  empresa_id: string;
  parte_id: string;
  tipo: string;
  titulo: string;
  datos: Record<string, unknown>;
  adjuntos: string[];
  cita_id: string | null;
  item_id: string | null;
  autor_id: string;
  creado_en: string;
  actualizado_en: string;
}

const COLUMNAS = `id, empresa_id, parte_id, tipo, titulo, datos, adjuntos,
  cita_id, item_id, autor_id, creado_en, actualizado_en`;

@Injectable()
export class RegistrosService {
  constructor(
    private readonly pg: PgService,
    private readonly capacidades: CapacidadesService,
  ) {}

  async listar(
    empresaId: string,
    parteId: string,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
    tipo?: string,
    limite = 50,
  ): Promise<RegistroParte[]> {
    await this.validarAcceso(empresaId, parteId, actor, false);
    if (tipo !== undefined) this.validarTipo(tipo);
    const tope = Math.min(Math.max(Number(limite) || 50, 1), 100);
    const params: unknown[] = [empresaId, parteId];
    let filtro = '';
    if (tipo) {
      params.push(tipo);
      filtro = ` AND tipo = $${params.length}`;
    }
    params.push(tope);
    return this.pg.query<RegistroFila>(
      `SELECT ${COLUMNAS}
         FROM identity.partes_registros
        WHERE empresa_id = $1 AND parte_id = $2 AND eliminado_en IS NULL${filtro}
        ORDER BY creado_en DESC
        LIMIT $${params.length}`,
      params,
    );
  }

  async obtener(
    empresaId: string,
    parteId: string,
    registroId: string,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
  ): Promise<RegistroParte> {
    await this.validarAcceso(empresaId, parteId, actor, false);
    return this.buscar(empresaId, parteId, registroId);
  }

  async crear(
    empresaId: string,
    parteId: string,
    input: CrearRegistroInput,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
  ): Promise<RegistroParte> {
    await this.validarAcceso(empresaId, parteId, actor, true);
    const tipo = this.validarTipo(input.tipo);
    await this.exigirCapacidad(empresaId, tipo);
    const titulo = this.validarTitulo(input.titulo);
    const datos = this.validarDatos(input.datos ?? {});
    const adjuntos = this.validarAdjuntos(input.adjuntos ?? []);
    const citaId = this.validarUuidOpcional(input.cita_id, 'cita_id');
    const itemId = this.validarUuidOpcional(input.item_id, 'item_id');

    const fila = await this.pg.queryOne<RegistroFila>(
      `INSERT INTO identity.partes_registros
         (empresa_id, parte_id, tipo, titulo, datos, adjuntos, cita_id, item_id, autor_id)
       VALUES ($1, $2, $3, $4, $5::jsonb, $6::jsonb, $7, $8, $9)
       RETURNING ${COLUMNAS}`,
      [empresaId, parteId, tipo, titulo, JSON.stringify(datos), JSON.stringify(adjuntos), citaId, itemId, actor.user_id],
    );
    if (!fila) throw new DomainError('REGISTRO_NO_CREADO', 'No se pudo guardar el registro.');
    return fila;
  }

  async actualizar(
    empresaId: string,
    parteId: string,
    registroId: string,
    input: ActualizarRegistroInput,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
  ): Promise<RegistroParte> {
    await this.validarAcceso(empresaId, parteId, actor, true);
    const actual = await this.buscar(empresaId, parteId, registroId);
    const tipo = input.tipo === undefined ? actual.tipo : this.validarTipo(input.tipo);
    if (tipo !== actual.tipo) await this.exigirCapacidad(empresaId, tipo);
    const titulo = input.titulo === undefined ? actual.titulo : this.validarTitulo(input.titulo);
    const datos = input.datos === undefined ? actual.datos : this.validarDatos(input.datos);
    const adjuntos =
      input.adjuntos === undefined ? actual.adjuntos : this.validarAdjuntos(input.adjuntos);
    const citaId =
      input.cita_id === undefined ? actual.cita_id : this.validarUuidOpcional(input.cita_id, 'cita_id');
    const itemId =
      input.item_id === undefined ? actual.item_id : this.validarUuidOpcional(input.item_id, 'item_id');

    const fila = await this.pg.queryOne<RegistroFila>(
      `UPDATE identity.partes_registros
          SET tipo = $1, titulo = $2, datos = $3::jsonb, adjuntos = $4::jsonb,
              cita_id = $5, item_id = $6, actualizado_en = NOW()
        WHERE id = $7 AND empresa_id = $8 AND parte_id = $9 AND eliminado_en IS NULL
        RETURNING ${COLUMNAS}`,
      [tipo, titulo, JSON.stringify(datos), JSON.stringify(adjuntos), citaId, itemId, registroId, empresaId, parteId],
    );
    if (!fila) throw new NotFoundError('Registro', registroId);
    return fila;
  }

  async eliminar(
    empresaId: string,
    parteId: string,
    registroId: string,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
  ): Promise<{ ok: true }> {
    await this.validarAcceso(empresaId, parteId, actor, true);
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE identity.partes_registros
          SET eliminado_en = NOW(), actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND parte_id = $3 AND eliminado_en IS NULL
        RETURNING id`,
      [registroId, empresaId, parteId],
    );
    if (!fila) throw new NotFoundError('Registro', registroId);
    return { ok: true };
  }

  private async buscar(
    empresaId: string,
    parteId: string,
    registroId: string,
  ): Promise<RegistroParte> {
    if (!UUID.test(registroId)) throw new NotFoundError('Registro', registroId);
    const fila = await this.pg.queryOne<RegistroFila>(
      `SELECT ${COLUMNAS}
         FROM identity.partes_registros
        WHERE id = $1 AND empresa_id = $2 AND parte_id = $3 AND eliminado_en IS NULL`,
      [registroId, empresaId, parteId],
    );
    if (!fila) throw new NotFoundError('Registro', registroId);
    return fila;
  }

  private async validarAcceso(
    empresaId: string,
    parteId: string,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
    escritura: boolean,
  ): Promise<void> {
    if (!UUID.test(empresaId) || !UUID.test(parteId)) {
      throw new DomainError('PARTE_INVALIDA', 'La empresa o la parte no son validas.');
    }
    const esGlobal = actor.rol === ROLES.SUPER_ADMIN || actor.rol === ROLES.ADMIN;
    if (!esGlobal) {
      if (!ROLES_ACCESO.includes(actor.rol)) {
        throw new ForbiddenError('El rol no tiene acceso a registros de partes.');
      }
      if (actor.tenant_id !== empresaId) {
        throw new ForbiddenError('Solo puedes operar registros de tu empresa.');
      }
      if (escritura && !ROLES_ACCESO.includes(actor.rol)) {
        throw new ForbiddenError('El rol no puede modificar registros.');
      }
    }
    const parte = await this.pg.queryOne<{ id: string }>(
      `SELECT id FROM identity.partes
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [parteId, empresaId],
    );
    if (!parte) throw new NotFoundError('Parte', parteId);
  }

  private async exigirCapacidad(empresaId: string, tipo: string): Promise<void> {
    const requerida = tipo === 'receta' ? CAPACIDADES.RECETAS : CAPACIDADES.HISTORIA_CLINICA;
    const filas = await this.capacidades.activasDeEmpresas([empresaId], [requerida]);
    if (!filas.some((fila) => fila.activo && fila.empresa_id === empresaId)) {
      throw new CapabilityDisabledError([requerida], empresaId);
    }
  }

  private validarTipo(valor: string): string {
    const tipo = String(valor ?? '').trim().toLowerCase();
    if (!TIPO.test(tipo)) {
      throw new DomainError('REGISTRO_TIPO_INVALIDO', 'El tipo de registro no es valido.');
    }
    return tipo;
  }

  private validarTitulo(valor: string | undefined): string {
    const titulo = String(valor ?? '').trim();
    if (titulo.length > 200) {
      throw new DomainError('REGISTRO_TITULO_INVALIDO', 'El titulo admite hasta 200 caracteres.');
    }
    return titulo;
  }

  private validarDatos(valor: Record<string, unknown>): Record<string, unknown> {
    if (!valor || typeof valor !== 'object' || Array.isArray(valor)) {
      throw new DomainError('REGISTRO_DATOS_INVALIDOS', 'Los datos deben ser un objeto JSON.');
    }
    if (Buffer.byteLength(JSON.stringify(valor), 'utf8') > MAX_DATOS_BYTES) {
      throw new DomainError('REGISTRO_DATOS_INVALIDOS', 'Los datos superan el limite de 64 KB.');
    }
    return valor;
  }

  private validarAdjuntos(valor: string[]): string[] {
    if (!Array.isArray(valor) || valor.length > MAX_ADJUNTOS) {
      throw new DomainError('REGISTRO_ADJUNTOS_INVALIDOS', `Adjuntos: maximo ${MAX_ADJUNTOS} objetos.`);
    }
    for (const adjunto of valor) {
      if (typeof adjunto !== 'string' || !UUID.test(adjunto)) {
        throw new DomainError('REGISTRO_ADJUNTOS_INVALIDOS', 'Cada adjunto debe ser un UUID de storage.');
      }
    }
    return valor;
  }

  private validarUuidOpcional(valor: string | null | undefined, campo: string): string | null {
    if (valor === undefined || valor === null || valor === '') return null;
    if (typeof valor !== 'string' || !UUID.test(valor)) {
      throw new DomainError('REGISTRO_REFERENCIA_INVALIDA', `${campo} debe ser un UUID.`);
    }
    return valor;
  }
}
