import { Injectable } from '@nestjs/common';
import {
  UsuariosService,
  Usuario,
} from '../usuarios/usuarios.service';
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

/**
 * Registro, login y refresh con JWT de corta duracion + refresh (doc 4.3).
 * Supabase-compatible: mismo formato HS256; se conmuta sin friccion.
 */
@Injectable()
export class AuthService {
  private readonly secreto = process.env.JWT_SECRET ?? 'dev_secret';
  private readonly accessTtl = process.env.JWT_ACCESS_TTL ?? JWT_ACCESS_TTL_DEFAULT;
  private readonly refreshTtl = process.env.JWT_REFRESH_TTL ?? JWT_REFRESH_TTL_DEFAULT;

  constructor(private readonly usuarios: UsuariosService) {}

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
    const usuario = await this.usuarios.encontrarPorId(payload.sub);
    if (!usuario) {
      throw new DomainError('TOKEN_INVALIDO', 'El usuario del token ya no existe.');
    }
    return this.sesionDe(usuario);
  }

  private sesionDe(usuario: Usuario): Sesion {
    const contexto = {
      id: usuario.id,
      email: usuario.correo,
      rol: usuario.rol,
      nombre: usuario.nombre,
      ...(usuario.tenant_id ? { tenant_id: usuario.tenant_id } : {}),
      ...(usuario.personal_id ? { personal_id: usuario.personal_id } : {}),
    };
    const access = crearAccessToken(contexto, this.secreto, this.accessTtl);
    const refresh = crearRefreshToken(contexto, this.secreto, this.refreshTtl);
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