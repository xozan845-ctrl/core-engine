import { Injectable } from '@nestjs/common';
import * as bcrypt from 'bcryptjs';
import { PgService, DomainError, ConflictError, ForbiddenError, NotFoundError, UnauthorizedError, Rol, esUuid } from '@core/shared';

/**
 * Hash bcrypt valido que no corresponde a ninguna contrasena util. Se usa para
 * que el login tarde lo mismo cuando el correo no existe (anti-enumeracion).
 */
export const HASH_DUMMY = '$2b$10$Zuut.NowjEKNBr01fvSyL.TByllBR0bPvb2XIiWiW5g8WjTwClWxO';

export interface Usuario {
  id: string;
  nombre: string;
  correo: string;
  rol: Rol;
  tenant_id?: string;
  personal_id?: string;
  participacion_porcentaje?: number;
  avatar_objeto_id?: string;
  avatar_url?: string;
  creado_en: string;
}

interface UsuarioRow extends Usuario {
  contrasena_hash: string;
}

/**
 * Registro, sesiones y perfiles (RF-01). Compatible con Supabase Auth:
 * identidad propia en local; en staging/prod se conmuta al Admin API de Supabase.
 */
@Injectable()
export class UsuariosService {
  private readonly storageUrl = process.env.STORAGE_SERVICE_URL ?? 'http://storage-service:3010';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';

  constructor(private readonly pg: PgService) {}

  async encontrarPorId(id: string): Promise<Usuario | null> {
    const fila = await this.pg.queryOne<UsuarioRow>(
      `SELECT id, nombre, correo, rol, tenant_id, personal_id, participacion_porcentaje, avatar_objeto_id, creado_en FROM identity.usuarios WHERE id = $1`,
      [id],
    );
    return fila ? this.sanear(fila) : null;
  }

  async encontrarPorCorreo(correo: string): Promise<UsuarioRow | null> {
    return this.pg.queryOne<UsuarioRow>(
      `SELECT id, nombre, correo, rol, tenant_id, personal_id, participacion_porcentaje, avatar_objeto_id, contrasena_hash, creado_en
       FROM identity.usuarios WHERE LOWER(correo) = LOWER($1)`,
      [correo.trim()],
    );
  }

  async crear(datos: {
    nombre: string;
    correo: string;
    contrasena: string;
    rol: Rol;
    tenant_id?: string;
    personal_id?: string;
  }): Promise<Usuario> {
    const correo = datos.correo.trim().toLowerCase();
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(correo)) {
      throw new DomainError('CORREO_INVALIDO', 'El correo no tiene un formato valido.');
    }
    if (datos.contrasena.length < 8) {
      throw new DomainError('CONTRASENA_DEBIL', 'La contrasena debe tener al menos 8 caracteres.');
    }
    const existente = await this.encontrarPorCorreo(correo);
    if (existente) {
      throw new ConflictError('Ya existe una cuenta con ese correo.');
    }
    const hash = await bcrypt.hash(datos.contrasena, 10);
    const fila = await this.pg.queryOne<UsuarioRow>(
      `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol, tenant_id, personal_id)
       VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6)
       RETURNING id, nombre, correo, rol, tenant_id, personal_id, avatar_objeto_id, creado_en`,
      [datos.nombre.trim(), correo, hash, datos.rol, datos.tenant_id ?? null, datos.personal_id ?? null],
    );
    if (!fila) throw new DomainError('REGISTRO_FALLIDO', 'No se pudo crear el usuario.');
    return this.sanear(fila);
  }

  async crearVendedorEnEmpresa(datos: {
    nombre: string;
    correo: string;
    contrasena: string;
    empresa_id: string;
    participacion_porcentaje: number;
  }): Promise<Usuario> {
    const correo = datos.correo.trim().toLowerCase();
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(correo)) {
      throw new DomainError('CORREO_INVALIDO', 'El correo no tiene un formato valido.');
    }
    if (datos.contrasena.length < 8) {
      throw new DomainError('CONTRASENA_DEBIL', 'La contrasena debe tener al menos 8 caracteres.');
    }
    const hash = await bcrypt.hash(datos.contrasena, 10);
    return this.pg.transaccion(async (client) => {
      const existente = await client.query(`SELECT 1 FROM identity.usuarios WHERE LOWER(correo)=LOWER($1)`, [correo]);
      if (existente.rowCount) throw new ConflictError('Ya existe una cuenta con ese correo.');
      const result = await client.query<UsuarioRow>(
        `INSERT INTO identity.usuarios (nombre, correo, contrasena_hash, rol, tenant_id, participacion_porcentaje)
         VALUES ($1,$2,$3,'vendedor',$4,$5)
         RETURNING id, nombre, correo, rol, tenant_id, personal_id, participacion_porcentaje, avatar_objeto_id, creado_en`,
        [datos.nombre.trim(), correo, hash, datos.empresa_id, datos.participacion_porcentaje],
      );
      const usuario = result.rows[0];
      await client.query(
        `INSERT INTO identity.empresa_miembros
         (empresa_id, usuario_id, rol, participacion_porcentaje)
         VALUES ($1,$2,'vendedor',$3)`,
        [datos.empresa_id, usuario.id, datos.participacion_porcentaje],
      );
      return this.sanear(usuario);
    });
  }

  async verificarCredenciales(correo: string, contrasena: string): Promise<Usuario> {
    const fila = await this.encontrarPorCorreo(correo);
    // Comparar siempre contra un hash, exista o no el correo: sin esto el
    // tiempo de respuesta distingue cuentas reales (~70 ms) de inexistentes
    // (~5 ms) y permite enumerar usuarios (R-DS-2).
    const hash = fila?.contrasena_hash ?? HASH_DUMMY;
    const ok = await bcrypt.compare(contrasena, hash);
    if (!fila || !ok) {
      throw new UnauthorizedError('Correo o contrasena incorrectos.');
    }
    return this.sanear(fila);
  }

  existeAdmin(): Promise<boolean> {
    return this.pg.queryOne(`SELECT 1 FROM identity.usuarios WHERE rol IN ('super_admin','admin') LIMIT 1`).then((r) => !!r);
  }

  contarVendedores(): Promise<number> {
    return this.pg
      .queryOne<{ n: string }>(`SELECT COUNT(*)::int AS n FROM identity.usuarios WHERE rol = 'vendedor'`)
      .then((r) => Number(r?.n ?? 0));
  }

  listarVendedores(): Promise<Usuario[]> {
    return this.pg
      .query<UsuarioRow>(`SELECT id, nombre, correo, rol, avatar_objeto_id, creado_en FROM identity.usuarios WHERE rol = 'vendedor'`)
      .then((filas) => filas.map((f) => this.sanear(f)));
  }

  /** Lista de usuarios (admin). Filtra opcionalmente por rol y tenant. */
  async listar(filtro: { rol?: Rol; tenant_id?: string } = {}): Promise<Usuario[]> {
    const where: string[] = [];
    const params: unknown[] = [];
    if (filtro.rol) {
      params.push(filtro.rol);
      where.push(`rol = $${params.length}`);
    }
    if (filtro.tenant_id) {
      params.push(filtro.tenant_id);
      where.push(`tenant_id = $${params.length}`);
    }
    const sql = `SELECT id, nombre, correo, rol, tenant_id, personal_id, avatar_objeto_id, creado_en
                 FROM identity.usuarios${where.length ? ` WHERE ${where.join(' AND ')}` : ''}
                 ORDER BY creado_en DESC LIMIT 500`;
    const filas = await this.pg.query<UsuarioRow>(sql, params);
    return filas.map((f) => this.sanear(f));
  }

  /** Cambia la contrasena verificando la actual (app: cambiar-contrasena). */
  async cambiarContrasena(id: string, actual: string, nueva: string): Promise<void> {
    const fila = await this.encontrarPorId(id);
    if (!fila) throw new NotFoundError('Usuario', id);
    const row = await this.encontrarPorCorreo(fila.correo);
    const ok = row ? await bcrypt.compare(actual, row.contrasena_hash) : false;
    if (!ok) throw new UnauthorizedError('La contrasena actual es incorrecta.');
    if (nueva.length < 8) {
      throw new DomainError('CONTRASENA_DEBIL', 'La nueva contrasena debe tener al menos 8 caracteres.');
    }
    const hash = await bcrypt.hash(nueva, 10);
    await this.pg.query(`UPDATE identity.usuarios SET contrasena_hash = $1 WHERE id = $2`, [hash, id]);
  }

  /** Vincula la ficha de personal del usuario (app: AuthUser.personalId). */
  async vincularPersonal(id: string, personal_id: string): Promise<void> {
    await this.pg.query(`UPDATE identity.usuarios SET personal_id = $1 WHERE id = $2`, [personal_id, id]);
  }

  /** Actualiza campos de perfil del usuario (app: nombre desde el dispositivo). */
  async actualizarPerfil(id: string, datos: { nombre?: string; correo?: string }): Promise<void> {
    const sets: string[] = [];
    const params: unknown[] = [];
    if (datos.nombre !== undefined) {
      params.push(datos.nombre.trim());
      sets.push(`nombre = $${params.length}`);
    }
    if (datos.correo !== undefined) {
      const correo = datos.correo.trim().toLowerCase();
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(correo)) {
        throw new DomainError('CORREO_INVALIDO', 'El correo no tiene un formato valido.');
      }
      params.push(correo);
      sets.push(`correo = $${params.length}`);
    }
    if (sets.length === 0) return;
    params.push(id);
    await this.pg.query(`UPDATE identity.usuarios SET ${sets.join(', ')} WHERE id = $${params.length}`, params);
  }

  /**
   * Asocia un avatar público ya confirmado en storage. El vendedor puede
   * editar el suyo; el empresario, los de su tenant; admin/super_admin, todos.
   */
  async actualizarAvatar(
    id: string,
    avatarObjetoId: string | null,
    actor: { user_id: string; rol: Rol; tenant_id?: string },
  ): Promise<Usuario> {
    const objetivo = await this.encontrarPorId(id);
    if (!objetivo) throw new NotFoundError('Usuario', id);
    const esGlobal = actor.rol === 'super_admin' || actor.rol === 'admin';
    const esPropio = actor.user_id === id;
    const esEmpresarioDelTenant =
      actor.rol === 'empresario' && !!actor.tenant_id && actor.tenant_id === objetivo.tenant_id;
    if (!esGlobal && !esPropio && !esEmpresarioDelTenant) {
      throw new ForbiddenError('No puedes cambiar el avatar de este usuario.');
    }
    if (avatarObjetoId !== null) {
      if (!esUuid(avatarObjetoId)) {
        throw new DomainError('AVATAR_INVALIDO', 'El avatar debe ser un objeto de storage válido.');
      }
      const tenantId = objetivo.tenant_id ?? actor.tenant_id;
      if (!tenantId) throw new DomainError('TENANT_REQUERIDO', 'El usuario no tiene tenant para el avatar.');
      await this.validarAvatar(tenantId, avatarObjetoId);
    }
    const fila = await this.pg.queryOne<UsuarioRow>(
      `UPDATE identity.usuarios SET avatar_objeto_id=$1, actualizado_en=NOW()
        WHERE id=$2
        RETURNING id, nombre, correo, rol, tenant_id, personal_id, participacion_porcentaje, avatar_objeto_id, creado_en`,
      [avatarObjetoId, id],
    );
    if (!fila) throw new NotFoundError('Usuario', id);
    return this.sanear(fila);
  }

  /** El avatar debe ser una imagen pública, activa y del tenant del usuario. */
  private async validarAvatar(tenantId: string, objetoId: string): Promise<void> {
    let respuesta: Response;
    try {
      respuesta = await fetch(
        `${this.storageUrl}/internal/storage/objetos/${encodeURIComponent(objetoId)}` +
          `?tenant_id=${encodeURIComponent(tenantId)}`,
        {
          headers: { 'x-internal-key': this.claveInterna },
          // R-RB-4: fallar rapido en vez de esperar el timeout del proxy.
          signal: AbortSignal.timeout(4000),
        },
      );
    } catch {
      throw new DomainError('AVATAR_NO_VERIFICABLE', 'No se pudo verificar el avatar.');
    }
    if (respuesta.status === 404) throw new NotFoundError('Avatar', objetoId);
    if (!respuesta.ok) {
      throw new DomainError('AVATAR_NO_VERIFICABLE', 'No se pudo verificar el avatar.');
    }
    const objeto = (await respuesta.json()) as {
      tenant_id?: string;
      estado?: string;
      publico?: boolean;
      content_type?: string;
    };
    if (
      objeto.tenant_id !== tenantId ||
      objeto.estado !== 'activo' ||
      objeto.publico !== true ||
      !String(objeto.content_type ?? '').startsWith('image/')
    ) {
      throw new DomainError(
        'AVATAR_INVALIDO',
        'El avatar debe ser una imagen pública y activa del tenant.',
      );
    }
  }

  private sanear(fila: UsuarioRow): Usuario {
    // los tokens de acceso nunca exponen el hash (minima exposicion de datos)
    return {
      id: fila.id,
      nombre: fila.nombre,
      correo: fila.correo,
      rol: fila.rol,
      ...(fila.tenant_id ? { tenant_id: fila.tenant_id } : {}),
      ...(fila.personal_id ? { personal_id: fila.personal_id } : {}),
      ...(fila.participacion_porcentaje !== undefined && fila.participacion_porcentaje !== null
        ? { participacion_porcentaje: fila.participacion_porcentaje } : {}),
      ...(fila.avatar_objeto_id
        ? {
            avatar_objeto_id: fila.avatar_objeto_id,
            avatar_url: `/api/v1/storage/public/objetos/${fila.avatar_objeto_id}`,
          }
        : {}),
      creado_en: fila.creado_en,
    };
  }
}
