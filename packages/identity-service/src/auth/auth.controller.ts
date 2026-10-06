import { Body, Controller, Get, Headers, Post, HttpCode } from '@nestjs/common';
import { ApiBody, ApiOperation, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
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
  UnauthorizedError,
  ForbiddenError,
} from '@core/shared';

export class RegistroRequestDto {
  @ApiProperty({ description: 'Nombre completo', minLength: 2, maxLength: 120, example: 'Ana Pérez' })
  @IsString()
  @Length(2, 120)
  nombre: string;

  /** require_tld:false admite TLDs de desarrollo (.test) y correos internos. */
  @ApiProperty({ description: 'Correo electrónico', example: 'ana@tienda.test' })
  @IsEmail({ require_tld: false })
  correo: string;

  @ApiProperty({ description: 'Contraseña (mínimo 8 caracteres)', minLength: 8 })
  @IsString()
  @MinLength(8)
  contrasena: string;

  /** Solo vendedor o comprador por registro publico (el admin se siembra). */
  @ApiProperty({ description: 'Rol registrable', enum: [...ROLES_REGISTRABLES], example: 'comprador' })
  @IsIn([...ROLES_REGISTRABLES])
  rol: (typeof ROLES)[keyof typeof ROLES];
}

export class LoginRequestDto {
  /** require_tld:false admite TLDs de desarrollo (.test) y correos internos. */
  @ApiProperty({ description: 'Correo electrónico', example: 'ana@tienda.test' })
  @IsEmail({ require_tld: false })
  correo: string;

  @ApiProperty({ description: 'Contraseña', example: 'secreto123' })
  @IsString()
  contrasena: string;
}

export class CrearUsuarioRequestDto {
  @ApiProperty({ description: 'Nombre completo', minLength: 2, maxLength: 120 })
  @IsString()
  @Length(2, 120)
  nombre: string;

  @ApiProperty({ description: 'Correo electrónico' })
  @IsEmail({ require_tld: false })
  correo: string;

  @ApiProperty({ description: 'Contraseña (mínimo 8 caracteres)', minLength: 8 })
  @IsString()
  @MinLength(8)
  contrasena: string;

  @ApiProperty({
    description: 'Rol del usuario',
    enum: [...ROLES_REGISTRABLES, ROLES.ADMIN, ROLES.LOGISTICA, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO],
  })
  @IsIn([...ROLES_REGISTRABLES, ROLES.ADMIN, ROLES.LOGISTICA, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO])
  rol: Rol;

  /** Tenant (organizacion) al que pertenece; obligatorio para roles de logistica. */
  @ApiProperty({ description: 'Tenant (organización); obligatorio para roles de logística', required: false })
  @IsOptional()
  @IsString()
  tenant_id?: string;

  /** Ficha de personal en field-service (logistica de campo). */
  @ApiProperty({ description: 'Id de la ficha de personal en field-service', required: false })
  @IsOptional()
  @IsString()
  personal_id?: string;
}

export class CambiarContrasenaRequestDto {
  @ApiProperty({ description: 'Contraseña actual' })
  @IsString()
  @MinLength(1)
  actual: string;

  @ApiProperty({ description: 'Contraseña nueva (mínimo 8 caracteres)', minLength: 8 })
  @IsString()
  @MinLength(8)
  nueva: string;
}

export class RestablecerContrasenaRequestDto {
  @ApiProperty({ description: 'Correo electrónico de la cuenta' })
  @IsEmail({ require_tld: false })
  correo: string;
}

export class VincularPersonalRequestDto {
  /** Id de la ficha de personal en field-service (logistica de campo). */
  @ApiProperty({ description: 'Id de la ficha de personal en field-service', minLength: 1, maxLength: 100 })
  @IsString()
  @Length(1, 100)
  personal_id: string;

  /** Nombre a reflejar en el perfil (opcional). */
  @ApiProperty({ description: 'Nombre a reflejar en el perfil', required: false, maxLength: 120 })
  @IsOptional()
  @IsString()
  @Length(1, 120)
  nombre?: string;
}

@ApiTags('Auth')
@Controller('api/v1/auth')
export class AuthController {
  constructor(
    private readonly auth: AuthService,
    private readonly usuarios: UsuariosService,
  ) {}

  /** POST /api/v1/auth/registro — alta de vendedor o comprador (Tabla 21). */
  @Post('registro')
  @ApiOperation({ summary: 'Registrar vendedor o comprador' })
  @ApiResponse({ status: 201, description: 'Sesión creada (JWT + refresh)' })
  async registrar(@Body() dto: RegistroRequestDto): Promise<Sesion> {
    return this.auth.registrar(dto);
  }

  /** POST /api/v1/auth/login — JWT + refresh (Tabla 21). */
  @HttpCode(200)
  @Post('login')
  @ApiOperation({ summary: 'Iniciar sesión (JWT + refresh)' })
  @ApiResponse({ status: 200, description: 'Sesión iniciada' })
  @ApiResponse({ status: 401, description: 'Credenciales inválidas' })
  async login(@Body() dto: LoginRequestDto): Promise<Sesion> {
    return this.auth.login(dto.correo, dto.contrasena);
  }

  /** POST /api/v1/auth/refresh — renueva sesion con token de refresco. */
  @Post('refresh')
  @ApiOperation({ summary: 'Renovar la sesión con refresh token' })
  @ApiBody({ schema: { type: 'object', required: ['refresh_token'], properties: { refresh_token: { type: 'string' } } } })
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
  @ApiOperation({ summary: 'Crear usuario (solo admin)' })
  @ApiResponse({ status: 201, description: 'Usuario creado' })
  @ApiResponse({ status: 403, description: 'Solo un administrador puede crear usuarios' })
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
  @ApiOperation({ summary: 'Cambiar la contraseña (autenticado)' })
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
  @ApiOperation({ summary: 'Solicitar restablecimiento de contraseña (público)' })
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
  @ApiOperation({ summary: 'Perfil de la sesión actual' })
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
  @ApiOperation({ summary: 'Vincular la ficha de personal del usuario autenticado' })
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
