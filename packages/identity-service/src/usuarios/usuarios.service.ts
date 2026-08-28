import { Injectable } from '@nestjs/common';
import * as bcrypt from 'bcryptjs';
import { PgService, DomainError, ConflictError, NotFoundError, UnauthorizedError, Rol } from '@core/shared';

export interface Usuario {
  id: string;
  nombre: string;
  correo: string;
  rol: Rol;
  tenant_id?: string;
  personal_id?: string;
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
  constructor(private readonly pg: PgService) {}

  async encontrarPorId(id: string): Promise<Usuario | null> {
    const fila = await this.pg.queryOne<UsuarioRow>(
      `SELECT id, nombre, correo, rol, tenant_id, personal_id, creado_en FROM identity.usuarios WHERE id = $1`,
      [id],
    );
    return fila ? this.sanear(fila) : null;
  }

  async encontrarPorCorreo(correo: string): Promise<UsuarioRow | null> {
    return this.pg.queryOne<UsuarioRow>(
      `SELECT id, nombre, correo, rol, tenant_id, personal_id, contrasena_hash, creado_en
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
       RETURNING id, nombre, correo, rol, tenant_id, personal_id, creado_en`,
      [datos.nombre.trim(), correo, hash, datos.rol, datos.tenant_id ?? null, datos.personal_id ?? null],
    );
    if (!fila) throw new DomainError('REGISTRO_FALLIDO', 'No se pudo crear el usuario.');
    return this.sanear(fila);
  }

  async verificarCredenciales(correo: string, contrasena: string): Promise<Usuario> {
    const fila = await this.encontrarPorCorreo(correo);
    if (!fila) {
      throw new UnauthorizedError('Correo o contrasena incorrectos.');
    }
    const ok = await bcrypt.compare(contrasena, fila.contrasena_hash);
    if (!ok) {
      throw new UnauthorizedError('Correo o contrasena incorrectos.');
    }
    return this.sanear(fila);
  }

  existeAdmin(): Promise<boolean> {
    return this.pg.queryOne(`SELECT 1 FROM identity.usuarios WHERE rol = 'admin' LIMIT 1`).then((r) => !!r);
  }

  contarVendedores(): Promise<number> {
    return this.pg
      .queryOne<{ n: string }>(`SELECT COUNT(*)::int AS n FROM identity.usuarios WHERE rol = 'vendedor'`)
      .then((r) => Number(r?.n ?? 0));
  }

  listarVendedores(): Promise<Usuario[]> {
    return this.pg
      .query<UsuarioRow>(`SELECT id, nombre, correo, rol, creado_en FROM identity.usuarios WHERE rol = 'vendedor'`)
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
    const sql = `SELECT id, nombre, correo, rol, tenant_id, personal_id, creado_en
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

  private sanear(fila: UsuarioRow): Usuario {
    // los tokens de acceso nunca exponen el hash (minima exposicion de datos)
    return {
      id: fila.id,
      nombre: fila.nombre,
      correo: fila.correo,
      rol: fila.rol,
      ...(fila.tenant_id ? { tenant_id: fila.tenant_id } : {}),
      ...(fila.personal_id ? { personal_id: fila.personal_id } : {}),
      creado_en: fila.creado_en,
    };
  }
}