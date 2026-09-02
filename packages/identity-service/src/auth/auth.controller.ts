import { Body, Controller, Get, Headers, Param, Post, HttpCode } from '@nestjs/common';
import { IsEmail, IsIn, IsOptional, IsString, Length, MinLength } from 'class-validator';
import {
  AuthService,
  Sesion,
} from './auth.service';
import {
  UsuariosService,
  Usuario,
} from '../usuarios/usuarios.service';
import {
  ROLES,
  ROLES_REGISTRABLES,
  ROLES_LOGISTICA,
  Rol,
  UsuarioContexto,
  UsuarioActual,
  usuarioDesdeHeaders,
  DomainError,
  NotFoundError,
  UnauthorizedError,
  ForbiddenError,
} from '@core/shared';

export class RegistroRequestDto {
  @IsString()
  @Length(2, 120)
  nombre: string;

  /** require_tld:false admite TLDs de desarrollo (.test) y correos internos. */
  @IsEmail({ require_tld: false })
  correo: string;

  @IsString()
  @MinLength(8)
  contrasena: string;

  /** Solo vendedor o comprador por registro publico (el admin se siembra). */
  @IsIn([...ROLES_REGISTRABLES])
  rol: (typeof ROLES)[keyof typeof ROLES];
}

export class LoginRequestDto {
  /** require_tld:false admite TLDs de desarrollo (.test) y correos internos. */
  @IsEmail({ require_tld: false })
  correo: string;

  @IsString()
  contrasena: string;
}

export class CrearUsuarioRequestDto {
  @IsString()
  @Length(2, 120)
  nombre: string;

  @IsEmail({ require_tld: false })
  correo: string;

  @IsString()
  @MinLength(8)
  contrasena: string;

  @IsIn([...ROLES_REGISTRABLES, ROLES.ADMIN, ROLES.LOGISTICA, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO])
  rol: Rol;

  /** Tenant (organizacion) al que pertenece; obligatorio para roles de logistica. */
  @IsOptional()
  @IsString()
  tenant_id?: string;

  /** Ficha de personal en field-service (logistica de campo). */
  @IsOptional()
  @IsString()
  personal_id?: string;
}

export class CambiarContrasenaRequestDto {
  @IsString()
  @MinLength(1)
  actual: string;

  @IsString()
  @MinLength(8)
  nueva: string;
}

export class RestablecerContrasenaRequestDto {
  @IsEmail({ require_tld: false })
  correo: string;
}

export class VincularPersonalRequestDto {
  /** Id de la ficha de personal en field-service (logistica de campo). */
  @IsString()
  @Length(1, 100)
  personal_id: string;

  /** Nombre a reflejar en el perfil (opcional). */
  @IsOptional()
  @IsString()
  @Length(1, 120)
  nombre?: string;
}

@Controller('api/v1/auth')
export class AuthController {
  constructor(
    private readonly auth: AuthService,
    private readonly usuarios: UsuariosService,
  ) {}

  /** POST /api/v1/auth/registro — alta de vendedor o comprador (Tabla 21). */
  @Post('registro')
  async registrar(@Body() dto: RegistroRequestDto): Promise<Sesion> {
    return this.auth.registrar(dto);
  }

  /** POST /api/v1/auth/login — JWT + refresh (Tabla 21). */
  @HttpCode(200)
  @Post('login')
  async login(@Body() dto: LoginRequestDto): Promise<Sesion> {
    return this.auth.login(dto.correo, dto.contrasena);
  }

  /** POST /api/v1/auth/refresh — renueva sesion con token de refresco. */
  @Post('refresh')
  async refresh(@Body() body: { refresh_token: string }): Promise<Sesion> {
    if (!body.refresh_token) {
      throw new DomainError('TOKEN_FALTANTE', 'El refresh_token es obligatorio.');
    }
    return this.auth.refrescar(body.refresh_token);
  }

  /**
   * POST /api/v1/auth/crear-usuario — alta administrativa (admin). Sustituye a la
   * Cloud Function `crearUsuario` de Firebase: crea el usuario y devuelve su id.
   */
  @Post('crear-usuario')
  async crearUsuario(
    @Body() dto: CrearUsuarioRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Usuario> {
    if (!usuario) throw new UnauthorizedError('Token invalido o ausente.');
    // Solo un administrador puede dar de alta usuarios (sustituye la Cloud
    // Function protegida de Firebase). Evita la creación arbitraria de roles.
    if (usuario.rol !== ROLES.ADMIN) {
      throw new ForbiddenError('Solo un administrador puede crear usuarios.');
    }
    if (ROLES_LOGISTICA.includes(dto.rol) && !dto.tenant_id) {
      throw new DomainError('TENANT_REQUERIDO', 'Los usuarios de logistica requieren tenant_id.');
    }
    return this.usuarios.crear({
      nombre: dto.nombre,
      correo: dto.correo,
      contrasena: dto.contrasena,
      rol: dto.rol,
      tenant_id: dto.tenant_id,
      personal_id: dto.personal_id,
    });
  }

  /** POST /api/v1/auth/cambiar-contrasena — cambio autenticado (app: cambiar-contrasena). */
  @Post('cambiar-contrasena')
  async cambiarContrasena(
    @Body() dto: CambiarContrasenaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<{ ok: true }> {
    if (!usuario) throw new UnauthorizedError('Token invalido o ausente.');
    await this.usuarios.cambiarContrasena(usuario.user_id, dto.actual, dto.nueva);
    return { ok: true };
  }

  /**
   * POST /api/v1/auth/restablecer-contrasena — publico (app: restablecer-contrasena).
   * MVP: verifica existencia y confirma la peticion; el envio de correo se
   * conecta a Supabase Auth/SMTP en staging/prod.
   */
  @Post('restablecer-contrasena')
  async restablecerContrasena(@Body() dto: RestablecerContrasenaRequestDto): Promise<{ ok: true }> {
    const existe = await this.usuarios.encontrarPorCorreo(dto.correo);
    if (!existe) {
      // No revelar si existe o no (OWASP A07): siempre responde ok.
      return { ok: true };
    }
    return { ok: true };
  }

  /** GET /api/v1/auth/me — perfil de la sesion actual. */
  @Get('me')
  async yo(@Headers() headers: Record<string, unknown>): Promise<UsuarioContexto> {
    const usuario = usuarioDesdeHeaders(headers as Record<string, string | string[] | undefined>);
    if (!usuario) {
      throw new UnauthorizedError('Token invalido o ausente.');
    }
    return usuario;
  }

  /**
   * POST /api/v1/auth/vincular-personal — vincula la ficha de personal del
   * usuario autenticado (app: UsuarioRepositoryPort.saveUsuario con personalId).
   * Sustituye el setDoc de Firestore `usuarios/{uid}` desde la app.
   */
  @Post('vincular-personal')
  async vincularPersonal(
    @Body() dto: VincularPersonalRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<{ ok: true }> {
    if (!usuario) throw new UnauthorizedError('Token invalido o ausente.');
    await this.usuarios.vincularPersonal(usuario.user_id, dto.personal_id);
    if (dto.nombre) {
      await this.usuarios.actualizarPerfil(usuario.user_id, { nombre: dto.nombre });
    }
    return { ok: true };
  }
}

