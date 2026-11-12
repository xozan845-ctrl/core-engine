import { Inject, Injectable } from '@nestjs/common';
import { createHash, randomBytes } from 'node:crypto';
import * as bcrypt from 'bcryptjs';
import {
  ConflictError,
  DomainError,
  EMAIL_PROVIDER,
  ForbiddenError,
  GoneError,
  Logger,
  NotFoundError,
  obtenerTienda,
  PgService,
  Rol,
  ROLES,
  ServiceUnavailableError,
  tiendasPorIds,
} from '@core/shared';
import type { EmailProvider } from '@core/shared';
import { AuthService, Sesion } from '../auth/auth.service';

const TOKEN_TTL_MS = 72 * 60 * 60 * 1000;
const CORREO = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const CODIGO = /^[a-z][a-z0-9_]{1,39}$/;
const ACCION = /^[a-z][a-z0-9_.]{1,79}$/;

export interface PermisosRol {
  version?: number;
  acciones: string[];
  recursos?: string[];
  condiciones?: Record<string, unknown>;
}

export interface CrearRolDatos {
  codigo: string;
  nombre: string;
  descripcion?: string;
  permisos: PermisosRol;
}

export interface CrearPuestoDatos {
  codigo: string;
  nombre: string;
  tipo?: string;
  tienda_id?: string;
  metadata?: Record<string, unknown>;
}

export interface AsignarPersonalDatos {
  usuario_id: string;
  rol_empresa_id: string;
  tienda_id?: string;
  puesto_id?: string;
  participacion_porcentaje?: number;
  metadata?: Record<string, unknown>;
}

export interface CrearInvitacionPersonalDatos {
  correo: string;
  nombre?: string;
  tipo?: 'invitacion' | 'contratacion';
  rol_empresa_id: string;
  tienda_id?: string;
  puesto_id?: string;
  metadata?: Record<string, unknown>;
}

export interface AceptarInvitacionPersonalDatos {
  nombre: string;
  contrasena: string;
}

export interface RolEmpresa {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  descripcion: string;
  permisos: PermisosRol;
  activo: boolean;
  creado_en: string;
  actualizado_en: string;
}

export interface PuestoEmpresa {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  tipo: string;
  tienda_id: string | null;
  metadata: Record<string, unknown>;
  activo: boolean;
  creado_en: string;
  actualizado_en: string;
}

export interface InvitacionPersonal {
  id: string;
  empresa_id: string;
  correo: string;
  nombre: string | null;
  tipo: 'invitacion' | 'contratacion';
  rol_empresa_id: string;
  rol_nombre: string;
  tienda_id: string | null;
  puesto_id: string | null;
  estado: string;
  expira_en: string;
  url_invitacion: string;
}

export interface AceptacionPersonal extends Sesion {
  usuario: Sesion['usuario'] & {
    rol_empresa_id: string;
    tienda_id: string | null;
    puesto_id: string | null;
  };
}

interface Actor {
  user_id: string;
  rol: Rol;
  tenant_id?: string;
}

interface InvitacionVigente {
  id: string;
  empresa_id: string;
  correo: string;
  nombre: string | null;
  tipo: 'invitacion' | 'contratacion';
  rol_empresa_id: string;
  rol_nombre: string;
  tienda_id: string | null;
  puesto_id: string | null;
  metadata: Record<string, unknown>;
  expira_en: string;
  estado: string;
  empresa_nombre: string;
}

function hashToken(token: string): string {
  return createHash('sha256').update(token).digest('hex');
}

function escaparHtml(valor: string): string {
  return valor
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function validarObjeto(valor: unknown, campo: string): Record<string, unknown> {
  if (!valor || typeof valor !== 'object' || Array.isArray(valor)) {
    throw new DomainError(`${campo.toUpperCase()}_INVALIDO`, `${campo} debe ser un objeto JSON.`);
  }
  return valor as Record<string, unknown>;
}

function validarPermisos(permisos: PermisosRol): PermisosRol {
  const objeto = validarObjeto(permisos, 'permisos');
  if (!Array.isArray(objeto.acciones) || objeto.acciones.length === 0 || objeto.acciones.length > 200) {
    throw new DomainError('PERMISOS_INVALIDOS', 'permisos.acciones debe contener entre 1 y 200 acciones.');
  }
  const acciones = [...new Set(objeto.acciones)];
  if (acciones.some((accion) => typeof accion !== 'string' || !ACCION.test(accion))) {
    throw new DomainError('PERMISOS_INVALIDOS', 'Cada acción debe usar el formato recurso.accion.');
  }
  if (objeto.recursos !== undefined && (!Array.isArray(objeto.recursos) || objeto.recursos.some((r) => typeof r !== 'string' || !CODIGO.test(r)))) {
    throw new DomainError('PERMISOS_INVALIDOS', 'permisos.recursos debe ser una lista de códigos válidos.');
  }
  if (objeto.condiciones !== undefined) validarObjeto(objeto.condiciones, 'permisos.condiciones');
  return {
    version: typeof objeto.version === 'number' ? objeto.version : 1,
    acciones,
    ...(objeto.recursos ? { recursos: [...new Set(objeto.recursos as string[])] } : {}),
    ...(objeto.condiciones ? { condiciones: objeto.condiciones as Record<string, unknown> } : {}),
  };
}

function validarCodigo(codigo: string, campo: string): string {
  const normalizado = codigo.trim().toLowerCase();
  if (!CODIGO.test(normalizado)) throw new DomainError(`${campo.toUpperCase()}_INVALIDO`, `${campo} no es válido.`);
  return normalizado;
}

function validarCorreo(correo: string): string {
  const normalizado = correo.trim().toLowerCase();
  if (!CORREO.test(normalizado)) throw new DomainError('CORREO_INVALIDO', 'El correo no tiene un formato válido.');
  return normalizado;
}

@Injectable()
export class PersonalService {
  private readonly frontendUrl = (process.env.FRONTEND_URL ?? 'http://localhost:5173').replace(/\/+$/, '');
  private readonly logger = Logger.create('identity.personal');

  constructor(
    private readonly pg: PgService,
    private readonly auth: AuthService,
    @Inject(EMAIL_PROVIDER) private readonly email: EmailProvider,
  ) {}

  async crearRol(actor: Actor, empresaId: string, datos: CrearRolDatos): Promise<RolEmpresa> {
    await this.assertEmpresario(actor, empresaId);
    const codigo = validarCodigo(datos.codigo, 'código');
    const nombre = datos.nombre.trim();
    if (nombre.length < 2 || nombre.length > 120) throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 2 y 120 caracteres.');
    const permisos = validarPermisos(datos.permisos);
    const fila = await this.pg.queryOne<RolEmpresa>(
      `INSERT INTO identity.roles_empresa (empresa_id, codigo, nombre, descripcion, permisos, creado_por)
       VALUES ($1,$2,$3,$4,$5::jsonb,$6)
       RETURNING id, empresa_id, codigo, nombre, descripcion, permisos, activo, creado_en, actualizado_en`,
      [empresaId, codigo, nombre, datos.descripcion?.trim() ?? '', JSON.stringify(permisos), actor.user_id],
    );
    if (!fila) throw new DomainError('ROL_FALLIDO', 'No se pudo crear el rol.');
    return fila;
  }

  async listarRoles(actor: Actor, empresaId: string): Promise<RolEmpresa[]> {
    await this.assertAccesoEmpresa(actor, empresaId);
    return this.pg.query<RolEmpresa>(
      `SELECT id, empresa_id, codigo, nombre, descripcion, permisos, activo, creado_en, actualizado_en
       FROM identity.roles_empresa WHERE empresa_id=$1 ORDER BY nombre`,
      [empresaId],
    );
  }

  async actualizarRol(actor: Actor, empresaId: string, rolId: string, activo: boolean): Promise<RolEmpresa> {
    await this.assertEmpresario(actor, empresaId);
    const fila = await this.pg.queryOne<RolEmpresa>(
      `UPDATE identity.roles_empresa SET activo=$1, actualizado_en=NOW()
       WHERE id=$2 AND empresa_id=$3
       RETURNING id, empresa_id, codigo, nombre, descripcion, permisos, activo, creado_en, actualizado_en`,
      [activo, rolId, empresaId],
    );
    if (!fila) throw new NotFoundError('Rol', rolId);
    return fila;
  }

  async crearPuesto(actor: Actor, empresaId: string, datos: CrearPuestoDatos): Promise<PuestoEmpresa> {
    await this.assertEmpresario(actor, empresaId);
    const codigo = validarCodigo(datos.codigo, 'código');
    const tipo = validarCodigo(datos.tipo ?? 'operativo', 'tipo');
    const nombre = datos.nombre.trim();
    if (nombre.length < 2 || nombre.length > 120) throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 2 y 120 caracteres.');
    if (datos.tienda_id) await this.assertTienda(empresaId, datos.tienda_id);
    validarObjeto(datos.metadata ?? {}, 'metadata');
    const fila = await this.pg.queryOne<PuestoEmpresa>(
      `INSERT INTO identity.puestos_empresa (empresa_id, codigo, nombre, tipo, tienda_id, metadata, creado_por)
       VALUES ($1,$2,$3,$4,$5,$6::jsonb,$7)
       RETURNING id, empresa_id, codigo, nombre, tipo, tienda_id, metadata, activo, creado_en, actualizado_en`,
      [empresaId, codigo, nombre, tipo, datos.tienda_id ?? null, JSON.stringify(datos.metadata ?? {}), actor.user_id],
    );
    if (!fila) throw new DomainError('PUESTO_FALLIDO', 'No se pudo crear el puesto.');
    return fila;
  }

  async listarPuestos(actor: Actor, empresaId: string): Promise<PuestoEmpresa[]> {
    await this.assertAccesoEmpresa(actor, empresaId);
    return this.pg.query<PuestoEmpresa>(
      `SELECT id, empresa_id, codigo, nombre, tipo, tienda_id, metadata, activo, creado_en, actualizado_en
       FROM identity.puestos_empresa WHERE empresa_id=$1 ORDER BY nombre`,
      [empresaId],
    );
  }

  async asignar(actor: Actor, empresaId: string, datos: AsignarPersonalDatos) {
    await this.assertEmpresario(actor, empresaId);
    await this.assertMiembro(empresaId, datos.usuario_id);
    await this.assertRol(empresaId, datos.rol_empresa_id);
    if (!datos.tienda_id && !datos.puesto_id) throw new DomainError('ALCANCE_REQUERIDO', 'La asignación debe incluir tienda o puesto.');
    if (datos.tienda_id) await this.assertTienda(empresaId, datos.tienda_id);
    if (datos.puesto_id) await this.assertPuesto(empresaId, datos.puesto_id, datos.tienda_id);
    validarObjeto(datos.metadata ?? {}, 'metadata');
    const fila = await this.pg.queryOne(
      `INSERT INTO identity.asignaciones_personal
         (empresa_id, usuario_id, rol_empresa_id, tienda_id, puesto_id,
          participacion_porcentaje, metadata, creado_por)
       VALUES ($1,$2,$3,$4,$5,$6,$7::jsonb,$8)
       RETURNING id, empresa_id, usuario_id, rol_empresa_id, tienda_id, puesto_id,
                 participacion_porcentaje, metadata, estado, inicia_en, termina_en, creado_en`,
      [empresaId, datos.usuario_id, datos.rol_empresa_id, datos.tienda_id ?? null,
       datos.puesto_id ?? null, datos.participacion_porcentaje ?? null,
       JSON.stringify(datos.metadata ?? {}), actor.user_id],
    );
    if (!fila) throw new DomainError('ASIGNACION_FALLIDA', 'No se pudo crear la asignación.');
    return fila;
  }

  async listarAsignaciones(actor: Actor, empresaId: string) {
    await this.assertAccesoEmpresa(actor, empresaId);
    const filas = await this.pg.query<{ tienda_id: string | null } & Record<string, unknown>>(
      `SELECT a.id, a.empresa_id, a.usuario_id, u.nombre AS usuario_nombre, u.correo AS usuario_correo,
              a.rol_empresa_id, r.codigo AS rol_codigo, r.nombre AS rol_nombre,
              a.tienda_id, a.puesto_id, p.nombre AS puesto_nombre,
              a.metadata, a.estado, a.inicia_en, a.termina_en, a.creado_en
       FROM identity.asignaciones_personal a
       JOIN identity.usuarios u ON u.id=a.usuario_id
       JOIN identity.roles_empresa r ON r.id=a.rol_empresa_id
       LEFT JOIN identity.puestos_empresa p ON p.id=a.puesto_id AND p.empresa_id=a.empresa_id
       WHERE a.empresa_id=$1 ORDER BY a.creado_en DESC`,
      [empresaId],
    );

    // stores es la autoridad de las tiendas: se resuelve por su API interna.
    const ids = [...new Set(filas.map((f) => f.tienda_id).filter((id): id is string => Boolean(id)))];
    if (ids.length === 0) return filas;
    const tiendas = await tiendasPorIds(ids);
    if (tiendas === null) {
      // Degradación controlada del listado: se conserva la asignación sin nombre.
      return filas.map((f) => (f.tienda_id ? { ...f, tienda_nombre: null } : f));
    }
    const nombres = new Map(tiendas.map((t) => [t.id, t.nombre]));
    return filas.map((f) => (f.tienda_id ? { ...f, tienda_nombre: nombres.get(f.tienda_id) ?? null } : f));
  }

  async crearInvitacion(actor: Actor, empresaId: string, datos: CrearInvitacionPersonalDatos): Promise<InvitacionPersonal> {
    await this.assertEmpresario(actor, empresaId);
    const correo = validarCorreo(datos.correo);
    await this.assertRol(empresaId, datos.rol_empresa_id, true);
    if (datos.tienda_id) await this.assertTienda(empresaId, datos.tienda_id);
    if (datos.puesto_id) await this.assertPuesto(empresaId, datos.puesto_id, datos.tienda_id);
    validarObjeto(datos.metadata ?? {}, 'metadata');
    const token = randomBytes(32).toString('base64url');
    const expira = new Date(Date.now() + TOKEN_TTL_MS);
    const fila = await this.pg.queryOne<InvitacionPersonal & { empresa_nombre: string }>(
      `WITH creada AS (
         INSERT INTO identity.invitaciones_personal
           (empresa_id, correo, nombre, tipo, rol_empresa_id, tienda_id, puesto_id, metadata, token_hash, expira_en, creada_por_usuario_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8::jsonb,$9,$10,$11)
         RETURNING id, empresa_id, correo, nombre, tipo, rol_empresa_id, tienda_id, puesto_id, estado, expira_en
       )
       SELECT c.*, r.nombre AS rol_nombre, e.nombre AS empresa_nombre
       FROM creada c JOIN identity.roles_empresa r ON r.id=c.rol_empresa_id
       JOIN identity.empresas e ON e.id=c.empresa_id`,
      [empresaId, correo, datos.nombre?.trim() || null, datos.tipo ?? 'invitacion', datos.rol_empresa_id, datos.tienda_id ?? null, datos.puesto_id ?? null, JSON.stringify(datos.metadata ?? {}), hashToken(token), expira, actor.user_id],
    );
    if (!fila) throw new ConflictError('No se pudo generar el enlace de incorporación.');
    const url = `${this.frontendUrl}/incorporacion/${encodeURIComponent(token)}`;
    await this.enviarInvitacion(fila, url);
    return { ...fila, url_invitacion: url };
  }

  /**
   * Envía el enlace de incorporación con el adapter compartido. Un fallo de
   * correo no invalida la invitación ni rompe la respuesta: el enlace sigue
   * disponible para reenviarlo (R-OB-1: nunca se registra el correo/token).
   */
  private async enviarInvitacion(
    fila: InvitacionPersonal & { empresa_nombre: string },
    url: string,
  ): Promise<void> {
    const nombre = fila.nombre?.trim() || 'Hola';
    const asunto = `Invitación a ${fila.empresa_nombre}`;
    const texto =
      `${nombre}, te invitaron a unirte a ${fila.empresa_nombre} como ${fila.rol_nombre}. ` +
      `Acepta la invitación (válida 72 horas): ${url}`;
    const html =
      `<p>${escaparHtml(nombre)}, te invitaron a unirte a ` +
      `<strong>${escaparHtml(fila.empresa_nombre)}</strong> como ${escaparHtml(fila.rol_nombre)}.</p>` +
      `<p><a href="${url}">Aceptar invitación</a> (válida 72 horas).</p>`;
    try {
      await this.email.send({ to: fila.correo, subject: asunto, text: texto, html });
    } catch (error) {
      const codigo = (error as { code?: unknown })?.code;
      this.logger.error({
        msg: 'email_invitacion_fallido',
        ...(typeof codigo === 'string' ? { codigo } : {}),
      });
    }
  }

  async consultarInvitacion(token: string) {
    const fila = await this.buscarVigente(token);
    return {
      valida: true as const,
      correo: fila.correo,
      nombre: fila.nombre,
      tipo: fila.tipo,
      empresa_nombre: fila.empresa_nombre,
      rol_nombre: fila.rol_nombre,
      tienda_id: fila.tienda_id,
      puesto_id: fila.puesto_id,
      expira_en: fila.expira_en,
    };
  }

  async aceptarInvitacion(token: string, datos: AceptarInvitacionPersonalDatos): Promise<AceptacionPersonal> {
    const nombre = datos.nombre?.trim();
    if (!nombre || nombre.length < 2 || nombre.length > 120) throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 2 y 120 caracteres.');
    if (!datos.contrasena || datos.contrasena.length < 8) throw new DomainError('CONTRASENA_DEBIL', 'La contraseña debe tener al menos 8 caracteres.');
    const resultado = await this.pg.transaccion(async (client) => {
      const fila = await this.buscarVigenteEnCliente(client, token);
      const existente = await client.query(`SELECT 1 FROM identity.usuarios WHERE LOWER(correo)=LOWER($1)`, [fila.correo]);
      if (existente.rowCount) throw new ConflictError('El correo ya tiene una cuenta. Inicia sesión para aceptar la incorporación.', { correo: fila.correo }, 'CORREO_YA_REGISTRADO');
      const hash = await bcrypt.hash(datos.contrasena, 10);
      const usuario = await client.query<{
        id: string;
        nombre: string;
        correo: string;
        rol: Rol;
        tenant_id: string;
        creado_en: string;
      }>(
        `INSERT INTO identity.usuarios (nombre, correo, contrasena_hash, rol, tenant_id)
         VALUES ($1,$2,$3,'operativo',$4)
         RETURNING id, nombre, correo, rol, tenant_id, creado_en`,
        [nombre, fila.correo, hash, fila.empresa_id],
      );
      const creado = usuario.rows[0];
      await client.query(
        `INSERT INTO identity.asignaciones_personal
           (empresa_id, usuario_id, rol_empresa_id, tienda_id, puesto_id, metadata, creado_por)
         VALUES ($1,$2,$3,$4,$5,$6::jsonb,$2)`,
        [fila.empresa_id, creado.id, fila.rol_empresa_id, fila.tienda_id, fila.puesto_id, JSON.stringify(fila.metadata ?? {})],
      );
      await client.query(`UPDATE identity.invitaciones_personal SET estado='aceptada', aceptada_en=NOW(), aceptada_por_usuario_id=$1 WHERE id=$2`, [creado.id, fila.id]);
      return { usuario: creado, rol_empresa_id: fila.rol_empresa_id, tienda_id: fila.tienda_id, puesto_id: fila.puesto_id };
    });
    const sesion = await this.auth.crearSesion({
      ...resultado.usuario,
      rol_empresa_id: resultado.rol_empresa_id,
    });
    return {
      ...sesion,
      usuario: { ...sesion.usuario, rol_empresa_id: resultado.rol_empresa_id, tienda_id: resultado.tienda_id, puesto_id: resultado.puesto_id },
    };
  }

  async listarInvitaciones(actor: Actor, empresaId: string) {
    await this.assertAccesoEmpresa(actor, empresaId);
    return this.pg.query(
      `SELECT i.id, i.empresa_id, i.correo, i.nombre, i.tipo, i.rol_empresa_id,
              r.nombre AS rol_nombre, i.tienda_id, i.puesto_id, i.estado, i.expira_en, i.creada_en
       FROM identity.invitaciones_personal i JOIN identity.roles_empresa r ON r.id=i.rol_empresa_id
       WHERE i.empresa_id=$1 ORDER BY i.creada_en DESC`,
      [empresaId],
    );
  }

  async revocarInvitacion(actor: Actor, empresaId: string, invitacionId: string) {
    await this.assertEmpresario(actor, empresaId);
    const fila = await this.pg.queryOne(`UPDATE identity.invitaciones_personal SET estado='revocada', revocada_en=NOW() WHERE id=$1 AND empresa_id=$2 AND estado='pendiente' RETURNING id`, [invitacionId, empresaId]);
    if (!fila) throw new NotFoundError('Invitación', invitacionId);
    return { ok: true as const };
  }

  private async assertAccesoEmpresa(actor: Actor, empresaId: string): Promise<void> {
    const global = actor.rol === ROLES.SUPER_ADMIN || actor.rol === ROLES.ADMIN;
    if (global) return;
    if (actor.rol !== ROLES.EMPRESARIO || actor.tenant_id !== empresaId) throw new ForbiddenError('No tienes acceso a esta empresa.');
    await this.assertEmpresario(actor, empresaId);
  }

  private async assertEmpresario(actor: Actor, empresaId: string): Promise<void> {
    const global = actor.rol === ROLES.SUPER_ADMIN || actor.rol === ROLES.ADMIN;
    if (global) return;
    if (actor.rol !== ROLES.EMPRESARIO || actor.tenant_id !== empresaId) throw new ForbiddenError('Solo el empresario activo puede administrar personal.');
    const miembro = await this.pg.queryOne(
      `SELECT 1 FROM identity.asignaciones_personal a
       JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
       WHERE a.empresa_id=$1 AND a.usuario_id=$2 AND r.codigo='empresario' AND a.estado='activa'`,
      [empresaId, actor.user_id],
    );
    if (!miembro) throw new ForbiddenError('El usuario no es empresario activo de esta empresa.');
  }

  private async assertRol(empresaId: string, rolId: string, activo = false): Promise<void> {
    const fila = await this.pg.queryOne<{ id: string }>(`SELECT id FROM identity.roles_empresa WHERE id=$1 AND empresa_id=$2${activo ? ' AND activo=true' : ''}`, [rolId, empresaId]);
    if (!fila) throw new NotFoundError('Rol', rolId);
  }

  private async assertPuesto(empresaId: string, puestoId: string, tiendaId?: string): Promise<void> {
    const fila = await this.pg.queryOne<{ id: string; tienda_id: string | null }>(`SELECT id, tienda_id FROM identity.puestos_empresa WHERE id=$1 AND empresa_id=$2 AND activo=true`, [puestoId, empresaId]);
    if (!fila) throw new NotFoundError('Puesto', puestoId);
    if (fila.tienda_id && tiendaId && fila.tienda_id !== tiendaId) throw new ConflictError('El puesto pertenece a otra tienda.');
  }

  private async assertTienda(empresaId: string, tiendaId: string): Promise<void> {
    const consulta = await obtenerTienda(tiendaId);
    if (consulta.estado === 'error') throw new ServiceUnavailableError('stores-service');
    if (consulta.estado !== 'ok' || consulta.tienda.empresa_id !== empresaId) {
      throw new NotFoundError('Tienda', tiendaId);
    }
  }

  private async assertMiembro(empresaId: string, usuarioId: string): Promise<void> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM identity.asignaciones_personal
       WHERE empresa_id=$1 AND usuario_id=$2 AND estado='activa'`,
      [empresaId, usuarioId],
    );
    if (!fila) throw new NotFoundError('Miembro', usuarioId);
  }

  private async buscarVigente(token: string): Promise<InvitacionVigente> {
    const fila = await this.pg.queryOne<InvitacionVigente>(`${SELECT_INVITACION} WHERE i.token_hash=$1 AND e.estado='activa'`, [hashToken(token)]);
    this.validarVigente(fila);
    return fila as InvitacionVigente;
  }

  private async buscarVigenteEnCliente(client: { query: <T>(sql: string, values?: unknown[]) => Promise<{ rows: T[]; rowCount?: number }> }, token: string): Promise<InvitacionVigente> {
    const result = await client.query<InvitacionVigente>(`${SELECT_INVITACION} WHERE i.token_hash=$1 AND e.estado='activa' FOR UPDATE OF i`, [hashToken(token)]);
    const fila = result.rows[0];
    this.validarVigente(fila);
    return fila as InvitacionVigente;
  }

  private validarVigente(fila: InvitacionVigente | null | undefined): void {
    if (!fila || fila.estado !== 'pendiente' || new Date(fila.expira_en).getTime() <= Date.now()) throw new GoneError('INVITACION_NO_VALIDA', 'La invitación no existe, expiró o ya fue utilizada.');
  }
}

const SELECT_INVITACION = `SELECT i.id, i.empresa_id, i.correo, i.nombre, i.tipo, i.rol_empresa_id,
       r.nombre AS rol_nombre, i.tienda_id, i.puesto_id, i.metadata, i.expira_en, i.estado,
       e.nombre AS empresa_nombre
FROM identity.invitaciones_personal i
JOIN identity.roles_empresa r ON r.id=i.rol_empresa_id
JOIN identity.empresas e ON e.id=i.empresa_id`;
