import { Body, Controller, Get, Param, Patch, Post } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import { IsBoolean, IsEmail, IsIn, IsInt, IsObject, IsOptional, IsString, IsUUID, Length, Matches, MinLength } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import {
  AceptarInvitacionPersonalDatos,
  AsignarPersonalDatos,
  CrearInvitacionPersonalDatos,
  CrearPuestoDatos,
  CrearRolDatos,
  PersonalService,
  PermisosRol,
} from './personal.service';

class PermisosRequestDto implements PermisosRol {
  @ApiProperty({ type: [String], example: ['catalogo.leer', 'ordenes.crear'] })
  @IsString({ each: true }) @MinLength(2, { each: true }) acciones: string[];
  @ApiProperty({ required: false, type: [String], example: ['tienda'] })
  @IsOptional() @IsString({ each: true }) recursos?: string[];
  @ApiProperty({ required: false, type: Object, additionalProperties: true })
  @IsOptional() @IsObject() condiciones?: Record<string, unknown>;
  @ApiProperty({ required: false, default: 1 }) @IsOptional() @IsInt() version?: number;
}

class CrearRolRequestDto implements CrearRolDatos {
  @ApiProperty({ pattern: '^[a-z][a-z0-9_]{1,39}$', example: 'estilista' }) @IsString() @Matches(/^[a-z][a-z0-9_]{1,39}$/) codigo: string;
  @ApiProperty({ minLength: 2, maxLength: 120, example: 'Estilista' }) @IsString() @Length(2, 120) nombre: string;
  @ApiProperty({ required: false, maxLength: 500 }) @IsOptional() @IsString() @Length(0, 500) descripcion?: string;
  @ApiProperty({ type: PermisosRequestDto }) @IsObject() permisos: PermisosRequestDto;
}

class CrearPuestoRequestDto implements CrearPuestoDatos {
  @ApiProperty({ pattern: '^[a-z][a-z0-9_]{1,39}$', example: 'sillon_1' }) @IsString() @Matches(/^[a-z][a-z0-9_]{1,39}$/) codigo: string;
  @ApiProperty({ minLength: 2, maxLength: 120, example: 'Sillón 1' }) @IsString() @Length(2, 120) nombre: string;
  @ApiProperty({ required: false, example: 'sillon' }) @IsOptional() @IsString() @Matches(/^[a-z][a-z0-9_]{1,39}$/) tipo?: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() tienda_id?: string;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}

class AsignarPersonalRequestDto implements AsignarPersonalDatos {
  @ApiProperty({ format: 'uuid' }) @IsUUID() usuario_id: string;
  @ApiProperty({ format: 'uuid' }) @IsUUID() rol_empresa_id: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() tienda_id?: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() puesto_id?: string;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}

class CrearInvitacionPersonalRequestDto implements CrearInvitacionPersonalDatos {
  @ApiProperty({ format: 'email' }) @IsEmail({ require_tld: false }) correo: string;
  @ApiProperty({ required: false, minLength: 2, maxLength: 120 }) @IsOptional() @IsString() @Length(2, 120) nombre?: string;
  @ApiProperty({ required: false, enum: ['invitacion', 'contratacion'], default: 'invitacion' }) @IsOptional() @IsIn(['invitacion', 'contratacion']) tipo?: 'invitacion' | 'contratacion';
  @ApiProperty({ format: 'uuid' }) @IsUUID() rol_empresa_id: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() tienda_id?: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() puesto_id?: string;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}

class EstadoRolRequestDto { @ApiProperty() @IsBoolean() activo: boolean; }
class AceptarInvitacionPersonalRequestDto implements AceptarInvitacionPersonalDatos {
  @ApiProperty({ minLength: 2, maxLength: 120 }) @IsString() @Length(2, 120) nombre: string;
  @ApiProperty({ minLength: 8 }) @IsString() @MinLength(8) contrasena: string;
}

@ApiTags('Personal') @ApiBearerAuth()
@Controller('api/v1/empresas/:empresaId')
export class PersonalController {
  constructor(private readonly personal: PersonalService) {}

  @Post('roles') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear un rol de personal con permisos JSON' })
  @ApiCreatedResponse({ description: 'Rol creado', type: Object })
  crearRol(@Param('empresaId') empresaId: string, @Body() dto: CrearRolRequestDto, @UsuarioActual() actor: UsuarioContexto) { return this.personal.crearRol(actor, empresaId, dto); }

  @Get('roles') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar roles configurables de la empresa' })
  @ApiOkResponse({ description: 'Roles de la empresa', type: Object })
  listarRoles(@Param('empresaId') empresaId: string, @UsuarioActual() actor: UsuarioContexto) { return this.personal.listarRoles(actor, empresaId); }

  @Patch('roles/:rolId') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Activar o desactivar un rol configurable' })
  @ApiOkResponse({ description: 'Rol actualizado', type: Object })
  actualizarRol(@Param('empresaId') empresaId: string, @Param('rolId') rolId: string, @Body() dto: EstadoRolRequestDto, @UsuarioActual() actor: UsuarioContexto) { return this.personal.actualizarRol(actor, empresaId, rolId, dto.activo); }

  @Post('puestos') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear un puesto operativo de la empresa' })
  @ApiCreatedResponse({ description: 'Puesto creado', type: Object })
  crearPuesto(@Param('empresaId') empresaId: string, @Body() dto: CrearPuestoRequestDto, @UsuarioActual() actor: UsuarioContexto) { return this.personal.crearPuesto(actor, empresaId, dto); }

  @Get('puestos') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar puestos operativos de la empresa' })
  @ApiOkResponse({ description: 'Puestos de la empresa', type: Object })
  listarPuestos(@Param('empresaId') empresaId: string, @UsuarioActual() actor: UsuarioContexto) { return this.personal.listarPuestos(actor, empresaId); }

  @Post('asignaciones-personal') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Asignar un miembro a un rol y tienda/puesto' })
  @ApiCreatedResponse({ description: 'Asignación creada', type: Object })
  asignar(@Param('empresaId') empresaId: string, @Body() dto: AsignarPersonalRequestDto, @UsuarioActual() actor: UsuarioContexto) { return this.personal.asignar(actor, empresaId, dto); }

  @Get('asignaciones-personal') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar asignaciones de personal' })
  @ApiOkResponse({ description: 'Asignaciones de la empresa', type: Object })
  listarAsignaciones(@Param('empresaId') empresaId: string, @UsuarioActual() actor: UsuarioContexto) { return this.personal.listarAsignaciones(actor, empresaId); }

  @Post('invitaciones/personal') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Generar enlace de invitación o contratación' })
  @ApiCreatedResponse({ description: 'Enlace de incorporación generado', type: Object })
  @ApiResponse({ status: 409, description: 'Correo o enlace duplicado' })
  crearInvitacion(@Param('empresaId') empresaId: string, @Body() dto: CrearInvitacionPersonalRequestDto, @UsuarioActual() actor: UsuarioContexto) { return this.personal.crearInvitacion(actor, empresaId, dto); }

  @Get('invitaciones/personal') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOkResponse({ description: 'Invitaciones de la empresa', type: Object })
  listarInvitaciones(@Param('empresaId') empresaId: string, @UsuarioActual() actor: UsuarioContexto) { return this.personal.listarInvitaciones(actor, empresaId); }

  @Post('invitaciones/personal/:invitacionId/revocar') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOkResponse({ description: 'Invitación revocada', type: Object })
  revocarInvitacion(@Param('empresaId') empresaId: string, @Param('invitacionId') invitacionId: string, @UsuarioActual() actor: UsuarioContexto) { return this.personal.revocarInvitacion(actor, empresaId, invitacionId); }
}

@ApiTags('Personal')
@Controller('api/v1/incorporaciones')
export class PersonalPublicController {
  constructor(private readonly personal: PersonalService) {}

  @Get(':token') @ApiOperation({ summary: 'Consultar un enlace de incorporación' })
  @ApiOkResponse({ description: 'Invitación vigente', type: Object })
  consultar(@Param('token') token: string) { return this.personal.consultarInvitacion(token); }

  @Post(':token/aceptar') @ApiOperation({ summary: 'Aceptar una invitación o contratación y crear la cuenta' })
  @ApiCreatedResponse({ description: 'Cuenta y membresía creadas', type: Object })
  aceptar(@Param('token') token: string, @Body() dto: AceptarInvitacionPersonalRequestDto) { return this.personal.aceptarInvitacion(token, dto); }
}
