import { Injectable } from '@nestjs/common';
import { createHash, randomBytes } from 'node:crypto';
import * as bcrypt from 'bcryptjs';
import {
  PgService,
  ConflictError,
  DomainError,
  ForbiddenError,
  GoneError,
  NotFoundError,
} from '@core/shared';
import { AuthService, Sesion } from '../auth/auth.service';
import { Usuario } from '../usuarios/usuarios.service';

const TOKEN_TTL_MS = 72 * 60 * 60 * 1000;
const CORREO = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export interface CrearInvitacionDatos {
  nombre?: string;
  correo: string;
  tienda_id: string;
  participacion_porcentaje: number;
}

export interface InvitacionPublica {
  valida: true;
  correo: string;
  empresa_nombre: string;
  tienda_nombre: string;
  participacion_porcentaje: number;
  expira_en: string;
}

export interface AceptarInvitacionDatos {
  nombre: string;
  contrasena: string;
}

export interface InvitacionCreada {
  id: string;
  empresa_id: string;
  tienda_id: string;
  tienda_nombre: string;
  correo: string;
  participacion_porcentaje: number;
  estado: string;
  expira_en: string;
  url_invitacion: string;
}

export interface VendedorEmpresa {
  id: string;
  nombre: string;
  correo: string;
  rol: string;
  tienda_id: string | null;
  tienda_nombre: string | null;
  participacion_porcentaje: number | null;
  estado: string;
}

export interface InvitacionResumen {
  id: string;
  correo: string;
  tienda_id: string;
  tienda_nombre: string;
  participacion_porcentaje: number;
  estado: string;
  expira_en: string;
}

function hashToken(token: string): string {
  return createHash('sha256').update(token).digest('hex');
}

function normalizarCorreo(correo: string): string {
  const normalizado = correo.trim().toLowerCase();
  if (!CORREO.test(normalizado)) {
    throw new DomainError('CORREO_INVALIDO', 'El correo no tiene un formato valido.');
  }
  return normalizado;
}

@Injectable()
export class InvitacionesService {
  private readonly frontendUrl = (
    process.env.FRONTEND_URL ?? 'http://localhost:5173'
  ).replace(/\/+$/, '');

  constructor(
    private readonly pg: PgService,
    private readonly auth: AuthService,
  ) {}

  async crear(
    actorId: string,
    empresaId: string,
    datos: CrearInvitacionDatos,
  ): Promise<InvitacionCreada> {
    const correo = normalizarCorreo(datos.correo);
    if (
      !Number.isInteger(datos.participacion_porcentaje) ||
      datos.participacion_porcentaje < 0 ||
      datos.participacion_porcentaje > 100
    ) {
      throw new DomainError(
        'PARTICIPACION_INVALIDA',
        'La participación debe ser un entero entre 0 y 100.',
      );
    }

    await this.assertEmpresario(actorId, empresaId);

    return this.pg.transaccion(async (client) => {
      const tienda = await client.query<{
        id: string;
        nombre: string;
        empresa_id: string;
        vendedor_id: string | null;
      }>(
        `SELECT id, nombre, empresa_id, vendedor_id
         FROM stores.tiendas WHERE id=$1 FOR UPDATE`,
        [datos.tienda_id],
      );
      const filaTienda = tienda.rows[0];
      if (!filaTienda || filaTienda.empresa_id !== empresaId) {
        throw new NotFoundError('Tienda', datos.tienda_id);
      }
      if (filaTienda.vendedor_id) {
        throw new ConflictError(
          'La tienda seleccionada ya tiene un vendedor asignado.',
          { tienda_id: datos.tienda_id },
          'TIENDA_YA_ASIGNADA',
        );
      }

      const miembro = await client.query(
        `SELECT 1
         FROM identity.empresa_miembros m
         JOIN identity.usuarios u ON u.id = m.usuario_id
         WHERE m.empresa_id=$1 AND LOWER(u.correo)=LOWER($2) AND m.estado='activo'
         LIMIT 1`,
        [empresaId, correo],
      );
      if (miembro.rowCount) {
        throw new ConflictError(
          'El correo ya tiene una membresía activa en esta empresa.',
          { correo },
          'YA_ES_MIEMBRO',
        );
      }

      const vendedorConTienda = await client.query(
        `SELECT 1
         FROM identity.usuarios u
         JOIN stores.tiendas t ON t.vendedor_id = u.id
         WHERE LOWER(u.correo)=LOWER($1)
         LIMIT 1`,
        [correo],
      );
      if (vendedorConTienda.rowCount) {
        throw new ConflictError(
          'El vendedor ya tiene una tienda asignada.',
          { correo },
          'VENDEDOR_YA_TIENE_TIENDA',
        );
      }

      const pendiente = await client.query(
        `SELECT 1 FROM identity.invitaciones_vendedor
         WHERE empresa_id=$1 AND tienda_id=$2 AND LOWER(correo)=LOWER($3)
           AND estado='pendiente'
         LIMIT 1`,
        [empresaId, datos.tienda_id, correo],
      );
      if (pendiente.rowCount) {
        throw new ConflictError(
          'Ya existe una invitación pendiente para este correo.',
          { correo },
          'INVITACION_PENDIENTE',
        );
      }

      const token = randomBytes(32).toString('base64url');
      const expira = new Date(Date.now() + TOKEN_TTL_MS);
      const creada = await client.query<{
        id: string;
        estado: string;
        expira_en: string;
      }>(
        `INSERT INTO identity.invitaciones_vendedor
           (empresa_id, tienda_id, correo, nombre, participacion_porcentaje,
            token_hash, expira_en, creada_por_usuario_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         RETURNING id, estado, expira_en`,
        [
          empresaId,
          datos.tienda_id,
          correo,
          datos.nombre?.trim() || null,
          datos.participacion_porcentaje,
          hashToken(token),
          expira,
          actorId,
        ],
      );
      const fila = creada.rows[0];

      return {
        id: fila.id,
        empresa_id: empresaId,
        tienda_id: datos.tienda_id,
        tienda_nombre: filaTienda.nombre,
        correo,
        participacion_porcentaje: datos.participacion_porcentaje,
        estado: fila.estado,
        expira_en: fila.expira_en,
        url_invitacion: `${this.frontendUrl}/registro/vendedor?token=${encodeURIComponent(token)}`,
      };
    });
  }

  async consultar(token: string): Promise<InvitacionPublica> {
    const fila = await this.buscarVigenteEnPg(token);
    return {
      valida: true,
      correo: fila.correo,
      empresa_nombre: fila.empresa_nombre,
      tienda_nombre: fila.tienda_nombre,
      participacion_porcentaje: fila.participacion_porcentaje,
      expira_en: fila.expira_en,
    };
  }

  async aceptar(
    token: string,
    datos: AceptarInvitacionDatos,
  ): Promise<Sesion & { usuario: Sesion['usuario'] & { tienda_id: string; tienda_nombre: string; participacion_porcentaje: number } }> {
    const nombre = datos.nombre?.trim();
    if (!nombre || nombre.length < 2 || nombre.length > 120) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre debe tener entre 2 y 120 caracteres.');
    }
    if (!datos.contrasena || datos.contrasena.length < 8) {
      throw new DomainError(
        'CONTRASENA_DEBIL',
        'La contraseña debe tener al menos 8 caracteres.',
      );
    }

    const resultado = await this.pg.transaccion(async (client) => {
      const fila = await this.buscarVigenteEnCliente(client, token);

      const existente = await client.query(
        `SELECT 1 FROM identity.usuarios WHERE LOWER(correo)=LOWER($1) LIMIT 1`,
        [fila.correo],
      );
      if (existente.rowCount) {
        throw new ConflictError(
          'El correo ya tiene una cuenta. Inicia sesión para aceptar la invitación.',
          { correo: fila.correo },
          'CORREO_YA_REGISTRADO',
        );
      }

      const hash = await bcrypt.hash(datos.contrasena, 10);
      const usuario = await client.query<Usuario>(
        `INSERT INTO identity.usuarios
           (nombre, correo, contrasena_hash, rol, tenant_id, participacion_porcentaje)
         VALUES ($1,$2,$3,'vendedor',$4,$5)
         RETURNING id, nombre, correo, rol, tenant_id, participacion_porcentaje, creado_en`,
        [nombre, fila.correo, hash, fila.empresa_id, fila.participacion_porcentaje],
      );
      const creado = usuario.rows[0];

      await client.query(
        `INSERT INTO identity.empresa_miembros
           (empresa_id, usuario_id, rol, participacion_porcentaje, estado)
         VALUES ($1,$2,'vendedor',$3,'activo')`,
        [fila.empresa_id, creado.id, fila.participacion_porcentaje],
      );

      const tienda = await client.query(
        `UPDATE stores.tiendas
         SET vendedor_id=$1, propietario_usuario_id=$1, participacion_porcentaje=$2
         WHERE id=$3 AND empresa_id=$4 AND vendedor_id IS NULL`,
        [creado.id, fila.participacion_porcentaje, fila.tienda_id, fila.empresa_id],
      );
      if (tienda.rowCount === 0) {
        throw new GoneError(
          'INVITACION_NO_VALIDA',
          'La tienda ya no está disponible; solicita una nueva invitación.',
        );
      }

      await client.query(
        `UPDATE identity.invitaciones_vendedor
         SET estado='aceptada', aceptada_en=NOW(), aceptada_por_usuario_id=$1
         WHERE id=$2`,
        [creado.id, fila.id],
      );

      return {
        usuario: creado,
        tienda_id: fila.tienda_id,
        tienda_nombre: fila.tienda_nombre,
        participacion_porcentaje: fila.participacion_porcentaje,
      };
    });

    const sesion = await this.auth.crearSesion(resultado.usuario);
    return {
      ...sesion,
      usuario: {
        ...sesion.usuario,
        tienda_id: resultado.tienda_id,
        tienda_nombre: resultado.tienda_nombre,
        participacion_porcentaje: resultado.participacion_porcentaje,
      },
    };
  }

  async listar(
    actorId: string,
    empresaId: string,
  ): Promise<{ vendedores: VendedorEmpresa[]; invitaciones: InvitacionResumen[] }> {
    await this.assertEmpresario(actorId, empresaId);

    const vendedores = await this.pg.query<VendedorEmpresa>(
      `SELECT u.id, u.nombre, u.correo, u.rol,
              t.id AS tienda_id, t.nombre AS tienda_nombre,
              m.participacion_porcentaje, m.estado
       FROM identity.empresa_miembros m
       JOIN identity.usuarios u ON u.id = m.usuario_id
       LEFT JOIN stores.tiendas t ON t.vendedor_id = u.id AND t.empresa_id = m.empresa_id
       WHERE m.empresa_id=$1 AND m.rol='vendedor'
       ORDER BY u.nombre`,
      [empresaId],
    );

    const invitaciones = await this.pg.query<InvitacionResumen>(
      `SELECT i.id, i.correo, i.tienda_id, t.nombre AS tienda_nombre,
              i.participacion_porcentaje,
              CASE WHEN i.estado='pendiente' AND i.expira_en <= NOW()
                   THEN 'expirada' ELSE i.estado END AS estado,
              i.expira_en
       FROM identity.invitaciones_vendedor i
       JOIN stores.tiendas t ON t.id = i.tienda_id
       WHERE i.empresa_id=$1
       ORDER BY i.creada_en DESC`,
      [empresaId],
    );

    return { vendedores, invitaciones };
  }

  async revocar(
    actorId: string,
    empresaId: string,
    invitacionId: string,
  ): Promise<{ ok: true }> {
    await this.assertEmpresario(actorId, empresaId);
    const fila = await this.pg.queryOne(
      `UPDATE identity.invitaciones_vendedor
       SET estado='revocada', revocada_en=NOW()
       WHERE id=$1 AND empresa_id=$2 AND estado='pendiente'
       RETURNING id`,
      [invitacionId, empresaId],
    );
    if (!fila) throw new NotFoundError('Invitación', invitacionId);
    return { ok: true };
  }

  private async assertEmpresario(actorId: string, empresaId: string): Promise<void> {
    const miembro = await this.pg.queryOne(
      `SELECT 1
       FROM identity.empresa_miembros m
       JOIN identity.empresas e ON e.id = m.empresa_id
       WHERE m.empresa_id=$1 AND m.usuario_id=$2 AND m.rol='empresario'
         AND m.estado='activo' AND e.estado='activa'`,
      [empresaId, actorId],
    );
    if (!miembro) {
      throw new ForbiddenError('Solo un empresario activo puede administrar invitaciones.');
    }
  }

  private async buscarVigenteEnPg(token: string): Promise<InvitacionVigente> {
    const fila = await this.pg.queryOne<InvitacionVigente>(
      `${SELECT_INVITACION} WHERE i.token_hash=$1 AND e.estado='activa'`,
      [hashToken(token)],
    );
    this.validarVigente(fila);
    return fila as InvitacionVigente;
  }

  private async buscarVigenteEnCliente(
    client: { query: <T>(texto: string, valores?: unknown[]) => Promise<{ rows: T[] }> },
    token: string,
  ): Promise<InvitacionVigente> {
    const fila = await client.query<InvitacionVigente>(
      `${SELECT_INVITACION} WHERE i.token_hash=$1 AND e.estado='activa' FOR UPDATE OF i, t`,
      [hashToken(token)],
    );
    const invitacion = fila.rows[0];
    this.validarVigente(invitacion);
    return invitacion;
  }

  private validarVigente(invitacion: InvitacionVigente | null | undefined): void {
    if (
      !invitacion ||
      invitacion.estado !== 'pendiente' ||
      new Date(invitacion.expira_en).getTime() <= Date.now() ||
      invitacion.tienda_empresa_id !== invitacion.empresa_id ||
      invitacion.tienda_vendedor_id
    ) {
      throw new GoneError(
        'INVITACION_NO_VALIDA',
        'La invitación no existe, expiró o ya fue utilizada.',
      );
    }
  }
}

interface InvitacionVigente {
  id: string;
  empresa_id: string;
  tienda_id: string;
  correo: string;
  participacion_porcentaje: number;
  expira_en: string;
  estado: string;
  empresa_nombre: string;
  tienda_nombre: string;
  tienda_empresa_id: string;
  tienda_vendedor_id: string | null;
}

const SELECT_INVITACION = `SELECT i.id, i.empresa_id, i.tienda_id, i.correo,
       i.participacion_porcentaje, i.expira_en, i.estado, e.nombre AS empresa_nombre,
       t.nombre AS tienda_nombre, t.empresa_id AS tienda_empresa_id,
       t.vendedor_id AS tienda_vendedor_id
FROM identity.invitaciones_vendedor i
JOIN identity.empresas e ON e.id = i.empresa_id
JOIN stores.tiendas t ON t.id = i.tienda_id`;
