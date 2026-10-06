import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  UsuariosService,
  Usuario,
} from '../usuarios/usuarios.service';
import { SesionesRepository } from './sesiones.repository';
import {
  crearAccessToken,
  crearRefreshToken,
  verificarToken,
  TokenPayload,
  DomainError,
  ROLES,
  JWT_ACCESS_TTL_DEFAULT,
  JWT_REFRESH_TTL_DEFAULT,
} from '@core/shared';

export interface Sesion {
  access_token: string;
  refresh_token: string;
  expira_en: number;
  usuario: {
    id: string;
    nombre: string;
    correo: string;
    rol: string;
    tenant_id?: string;
    personal_id?: string;
  };
}

/** Convierte un TTL tipo `900s`/`7d` a milisegundos (para la expiracion de la sesion). */
function ttlAMs(ttl: string): number {
  const match = /^(\d+)([smhd])$/.exec(ttl.trim());
  if (!match) return 7 * 24 * 60 * 60 * 1000;
  const valor = Number(match[1]);
  const unidades: Record<string, number> = { s: 1000, m: 60_000, h: 3_600_000, d: 86_400_000 };
  return valor * (unidades[match[2]] ?? 1000);
}

/**
 * Registro, login y refresh con JWT de corta duracion + refresh (doc 4.3).
 * Supabase-compatible: mismo formato HS256; se conmuta sin friccion.
 *
 * Cada sesion emitida se persiste en `identity.sesiones` por el `jti` del
 * refresh token, de modo que el logout la revoca (R-GW-4, ADR-18): el refresh
 * solo renueva mientras la sesion siga activa.
 */
@Injectable()
export class AuthService {
  private readonly secreto = process.env.JWT_SECRET ?? 'dev_secret';
  private readonly accessTtl = process.env.JWT_ACCESS_TTL ?? JWT_ACCESS_TTL_DEFAULT;
  private readonly refreshTtl = process.env.JWT_REFRESH_TTL ?? JWT_REFRESH_TTL_DEFAULT;

  constructor(
    private readonly usuarios: UsuariosService,
    private readonly sesiones: SesionesRepository,
  ) {}

  async registrar(datos: { nombre: string; correo: string; contrasena: string; rol: string }): Promise<Sesion> {
    if (![ROLES.VENDEDOR, ROLES.COMPRADOR].includes(datos.rol as 'vendedor' | 'comprador')) {
      throw new DomainError('ROL_NO_PERMITIDO', 'El registro publico solo admite vendedor o comprador.');
    }
    const usuario = await this.usuarios.crear({
      nombre: datos.nombre,
      correo: datos.correo,
      contrasena: datos.contrasena,
      rol: datos.rol as 'vendedor' | 'comprador',
    });
    return this.sesionDe(usuario);
  }

  async login(correo: string, contrasena: string): Promise<Sesion> {
    const usuario = await this.usuarios.verificarCredenciales(correo, contrasena);
    return this.sesionDe(usuario);
  }

  async refrescar(refreshToken: string): Promise<Sesion> {
    let payload: TokenPayload;
    try {
      payload = verificarToken<TokenPayload>(refreshToken, this.secreto);
    } catch {
      throw new DomainError('TOKEN_INVALIDO', 'El refresh_token es invalido o expiro.');
    }
    if (payload.tipo !== 'refresh') {
      throw new DomainError('TOKEN_INVALIDO', 'El token no es de refresco.');
    }
    if (!payload.jti) {
      throw new DomainError('TOKEN_INVALIDO', 'El token no tiene una sesion asociada.');
    }
    const sesion = await this.sesiones.activaPorJti(payload.jti);
    if (!sesion || sesion.usuario_id !== payload.sub) {
      throw new DomainError('SESION_REVOCADA', 'La sesion fue cerrada; inicia sesion de nuevo.');
    }
    const usuario = await this.usuarios.encontrarPorId(payload.sub);
    if (!usuario) {
      throw new DomainError('TOKEN_INVALIDO', 'El usuario del token ya no existe.');
    }
    return this.emitir(usuario, payload.jti);
  }

  /**
   * Cierra sesion (R-GW-4): revoca la sesion del refresh presentado o, si no
   * se indica, todas las del usuario. Idempotente (un token invalido no revoca
   * nada y no falla). Devuelve cuantas sesiones quedaron revocadas.
   */
  async cerrarSesion(usuarioId: string, refreshToken?: string): Promise<number> {
    if (!refreshToken) {
      return this.sesiones.revocarTodas(usuarioId);
    }
    try {
      const payload = verificarToken<TokenPayload>(refreshToken, this.secreto);
      if (payload.tipo === 'refresh' && payload.jti && payload.sub === usuarioId) {
        return this.sesiones.revocar(payload.jti, usuarioId);
      }
    } catch {
      // token invalido/expirado: no hay nada que revocar (logout idempotente)
    }
    return 0;
  }

  private async sesionDe(usuario: Usuario): Promise<Sesion> {
    const jti = randomUUID();
    const expiraEn = new Date(Date.now() + ttlAMs(this.refreshTtl));
    await this.sesiones.crear(usuario.id, jti, expiraEn);
    return this.emitir(usuario, jti);
  }

  private emitir(usuario: Usuario, jti: string): Sesion {
    const contexto = {
      id: usuario.id,
      email: usuario.correo,
      rol: usuario.rol,
      nombre: usuario.nombre,
      ...(usuario.tenant_id ? { tenant_id: usuario.tenant_id } : {}),
      ...(usuario.personal_id ? { personal_id: usuario.personal_id } : {}),
    };
    const access = crearAccessToken(contexto, this.secreto, this.accessTtl);
    const refresh = crearRefreshToken(contexto, this.secreto, this.refreshTtl, jti);
    return {
      access_token: access,
      refresh_token: refresh,
      expira_en: 900,
      usuario: {
        id: usuario.id,
        nombre: usuario.nombre,
        correo: usuario.correo,
        rol: usuario.rol,
        ...(usuario.tenant_id ? { tenant_id: usuario.tenant_id } : {}),
        ...(usuario.personal_id ? { personal_id: usuario.personal_id } : {}),
      },
    };
  }
}
