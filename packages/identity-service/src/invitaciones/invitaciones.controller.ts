import {
  Body,
  Controller,
  Delete,
  Get,
  Param,
  Post,
} from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiResponse,
  ApiTags,
} from '@nestjs/swagger';
import {
  IsEmail,
  IsInt,
  IsOptional,
  IsString,
  IsUUID,
  Length,
  Max,
  Min,
  MinLength,
} from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import {
  AceptarInvitacionDatos,
  CrearInvitacionDatos,
  InvitacionesService,
} from './invitaciones.service';

class CrearInvitacionRequestDto implements CrearInvitacionDatos {
  @ApiProperty({ format: 'email', example: 'vendedor@correo.com' })
  @IsEmail({ require_tld: false })
  correo: string;

  @ApiProperty({ required: false, minLength: 2, maxLength: 120 })
  @IsOptional()
  @IsString()
  @Length(2, 120)
  nombre?: string;

  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  tienda_id: string;

  @ApiProperty({ minimum: 0, maximum: 100, example: 25 })
  @IsInt()
  @Min(0)
  @Max(100)
  participacion_porcentaje: number;
}

class AceptarInvitacionRequestDto implements AceptarInvitacionDatos {
  @ApiProperty({ minLength: 2, maxLength: 120, example: 'Ana Pérez' })
  @IsString()
  @Length(2, 120)
  nombre: string;

  @ApiProperty({ minLength: 8, example: 'UnaClaveSegura123' })
  @IsString()
  @MinLength(8)
  contrasena: string;
}

class InvitacionCreadaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) tienda_id: string;
  @ApiProperty() tienda_nombre: string;
  @ApiProperty() correo: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) participacion_porcentaje: number;
  @ApiProperty() estado: string;
  @ApiProperty({ format: 'date-time' }) expira_en: string;
  @ApiProperty() url_invitacion: string;
}

class InvitacionPublicaResponseDto {
  @ApiProperty() valida: boolean;
  @ApiProperty() correo: string;
  @ApiProperty() empresa_nombre: string;
  @ApiProperty() tienda_nombre: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) participacion_porcentaje: number;
  @ApiProperty({ format: 'date-time' }) expira_en: string;
}

class VendedorEmpresaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty() nombre: string;
  @ApiProperty() correo: string;
  @ApiProperty() rol: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) tienda_id: string | null;
  @ApiProperty({ required: false, nullable: true }) tienda_nombre: string | null;
  @ApiProperty({ required: false, nullable: true, minimum: 0, maximum: 100 })
  participacion_porcentaje: number | null;
  @ApiProperty() estado: string;
}

class InvitacionResumenResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty() correo: string;
  @ApiProperty({ format: 'uuid' }) tienda_id: string;
  @ApiProperty() tienda_nombre: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) participacion_porcentaje: number;
  @ApiProperty() estado: string;
  @ApiProperty({ format: 'date-time' }) expira_en: string;
}

class ListarVendedoresResponseDto {
  @ApiProperty({ type: [VendedorEmpresaResponseDto] }) vendedores: VendedorEmpresaResponseDto[];
  @ApiProperty({ type: [InvitacionResumenResponseDto] })
  invitaciones: InvitacionResumenResponseDto[];
}

class AceptarInvitacionResponseDto {
  @ApiProperty() access_token: string;
  @ApiProperty() refresh_token: string;
  @ApiProperty() expira_en: number;
  @ApiProperty({
    type: Object,
    description: 'Usuario vendedor con tienda y participación asignadas',
  })
  usuario: Record<string, unknown>;
}

@ApiTags('Invitaciones')
@ApiBearerAuth()
@Controller('api/v1/empresas')
export class InvitacionesEmpresaController {
  constructor(private readonly invitaciones: InvitacionesService) {}

  @Post(':empresaId/invitaciones/vendedores')
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Generar una invitación para un futuro vendedor' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiCreatedResponse({ type: InvitacionCreadaResponseDto, description: 'Invitación generada' })
  @ApiResponse({ status: 409, description: 'Tienda asignada, invitación duplicada o correo ya miembro' })
  crear(
    @Param('empresaId') empresaId: string,
    @Body() dto: CrearInvitacionRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.invitaciones.crear(usuario.user_id, empresaId, dto);
  }

  @Get(':empresaId/vendedores')
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar vendedores activos e invitaciones de la empresa' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiOkResponse({
    type: ListarVendedoresResponseDto,
    description: 'Vendedores e invitaciones de la empresa',
  })
  listar(
    @Param('empresaId') empresaId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.invitaciones.listar(usuario.user_id, empresaId);
  }

  @Delete(':empresaId/invitaciones/vendedores/:invitacionId')
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Revocar una invitación pendiente' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'invitacionId', format: 'uuid' })
  @ApiOkResponse({ description: 'Invitación revocada' })
  revocar(
    @Param('empresaId') empresaId: string,
    @Param('invitacionId') invitacionId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.invitaciones.revocar(usuario.user_id, empresaId, invitacionId);
  }
}

@ApiTags('Invitaciones')
@Controller('api/v1/invitaciones/vendedores')
export class InvitacionesPublicasController {
  constructor(private readonly invitaciones: InvitacionesService) {}

  @Get(':token')
  @ApiOperation({ summary: 'Consultar una invitación por token (público)' })
  @ApiParam({ name: 'token' })
  @ApiOkResponse({ type: InvitacionPublicaResponseDto, description: 'Invitación válida' })
  @ApiResponse({ status: 410, description: 'Invitación inexistente, expirada o utilizada' })
  consultar(@Param('token') token: string) {
    return this.invitaciones.consultar(token);
  }

  @Post(':token/aceptar')
  @ApiOperation({ summary: 'Aceptar una invitación y crear la cuenta del vendedor (público)' })
  @ApiParam({ name: 'token' })
  @ApiCreatedResponse({
    type: AceptarInvitacionResponseDto,
    description: 'Vendedor creado y sesión iniciada',
  })
  @ApiResponse({ status: 410, description: 'Invitación inexistente, expirada o utilizada' })
  @ApiResponse({ status: 409, description: 'El correo ya tiene una cuenta o la tienda ya fue asignada' })
  aceptar(
    @Param('token') token: string,
    @Body() dto: AceptarInvitacionRequestDto,
  ) {
    return this.invitaciones.aceptar(token, dto);
  }
}
