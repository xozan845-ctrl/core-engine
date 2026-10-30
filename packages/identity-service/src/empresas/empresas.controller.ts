import { Body, Controller, Get, Param, Patch, Post, Res } from '@nestjs/common';
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
import type { Response } from 'express';
import { IsEmail, IsIn, IsInt, IsOptional, IsString, IsUUID, Length, Max, Min, MinLength } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { EmpresasService, ModalidadLogistica, TipoEmpresa } from './empresas.service';

class PerfilEmpresaRequestDto {
  @ApiProperty({ required: false, maxLength: 30, description: 'RUC opcional' })
  @IsOptional() @IsString() @Length(0, 30) ruc?: string;
  @ApiProperty({ required: false, maxLength: 160 }) @IsOptional() @IsString() @Length(0, 160) razon_social?: string;
  @ApiProperty({ required: false, maxLength: 160 }) @IsOptional() @IsString() @Length(0, 160) nombre_comercial?: string;
  @ApiProperty({ required: false, maxLength: 1000 }) @IsOptional() @IsString() @Length(0, 1000) descripcion?: string;
  @ApiProperty({ required: false, maxLength: 40 }) @IsOptional() @IsString() @Length(0, 40) telefono?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsEmail({ require_tld: false }) correo_contacto?: string;
  @ApiProperty({ required: false, maxLength: 200 }) @IsOptional() @IsString() @Length(0, 200) sitio_web?: string;
  @ApiProperty({ required: false, maxLength: 300 }) @IsOptional() @IsString() @Length(0, 300) direccion?: string;
  @ApiProperty({ required: false, maxLength: 80 }) @IsOptional() @IsString() @Length(0, 80) ciudad?: string;
  @ApiProperty({ required: false, maxLength: 80 }) @IsOptional() @IsString() @Length(0, 80) departamento?: string;
  @ApiProperty({ required: false, maxLength: 80, default: 'Nicaragua' }) @IsOptional() @IsString() @Length(0, 80) pais?: string;
  @ApiProperty({ required: false, minLength: 3, maxLength: 3, default: 'NIO' }) @IsOptional() @IsString() @Length(3, 3) moneda?: string;
  @ApiProperty({ required: false, maxLength: 64, default: 'America/Managua' }) @IsOptional() @IsString() @Length(0, 64) zona_horaria?: string;
  @ApiProperty({ required: false, maxLength: 80 }) @IsOptional() @IsString() @Length(0, 80) sector?: string;
  @ApiProperty({ required: false, maxLength: 160 }) @IsOptional() @IsString() @Length(0, 160) representante_legal?: string;
  @ApiProperty({ required: false, maxLength: 120 }) @IsOptional() @IsString() @Length(0, 120) horario_atencion?: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid', description: 'Objeto de storage confirmado como imagen pública' })
  @IsOptional() @IsUUID() logo_objeto_id?: string | null;
}

class CrearEmpresaRequestDto extends PerfilEmpresaRequestDto {
  @ApiProperty({ minLength: 2, maxLength: 160 }) @IsString() @Length(2, 160) nombre: string;
  @ApiProperty({
    required: false,
    minLength: 2,
    maxLength: 80,
    description: 'Slug público. Si se omite se deriva del nombre y se resuelven colisiones con sufijos.',
    example: 'distribuidora-piloto',
  })
  @IsOptional()
  @IsString()
  @Length(2, 80)
  slug?: string;
  @ApiProperty({ enum: ['plataforma', 'propia', 'hibrida'] }) @IsIn(['plataforma', 'propia', 'hibrida']) modalidad_logistica: ModalidadLogistica;
}
class CrearVendedorRequestDto {
  @ApiProperty() @IsString() @Length(2, 120) nombre: string;
  @ApiProperty() @IsEmail({ require_tld: false }) correo: string;
  @ApiProperty({ minLength: 8 }) @IsString() @MinLength(8) contrasena: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) @IsInt() @Min(0) @Max(100) participacion_porcentaje: number;
}
class ActualizarModalidadLogisticaRequestDto {
  @ApiProperty({ enum: ['plataforma', 'propia', 'hibrida'] })
  @IsIn(['plataforma', 'propia', 'hibrida'])
  modalidad_logistica: ModalidadLogistica;
}
class ActualizarSlugRequestDto {
  @ApiProperty({ minLength: 2, maxLength: 80, example: 'distribuidora-piloto' })
  @IsString()
  @Length(2, 80)
  slug: string;
}
class ActualizarTipoEmpresaRequestDto {
  @ApiProperty({ enum: ['comercial', 'logistica', 'plataforma'] })
  @IsIn(['comercial', 'logistica', 'plataforma'])
  tipo_empresa: TipoEmpresa;
}
export class EmpresaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty() nombre: string;
  // `required: false` mantiene el cambio como aditivo para el gate R-C-7;
  // el servicio siempre devuelve el slug en runtime.
  @ApiProperty({ required: false, description: 'Slug público actual', example: 'distribuidora-piloto' }) slug: string;
  @ApiProperty({ enum: ['plataforma', 'propia', 'hibrida'] }) modalidad_logistica: ModalidadLogistica;
  @ApiProperty({
    enum: ['comercial', 'logistica', 'plataforma'],
    required: false,
    description: 'Tipo de empresa; solo las comerciales tienen catálogo propio (ADR-27)',
  })
  tipo_empresa: TipoEmpresa;
  @ApiProperty() estado: string;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty() creado_en: string;
  @ApiProperty({ required: false, nullable: true }) ruc?: string | null;
  @ApiProperty({ required: false }) razon_social?: string;
  @ApiProperty({ required: false }) nombre_comercial?: string;
  @ApiProperty({ required: false }) descripcion?: string;
  @ApiProperty({ required: false }) telefono?: string;
  @ApiProperty({ required: false }) correo_contacto?: string;
  @ApiProperty({ required: false }) sitio_web?: string;
  @ApiProperty({ required: false }) direccion?: string;
  @ApiProperty({ required: false }) ciudad?: string;
  @ApiProperty({ required: false }) departamento?: string;
  @ApiProperty({ required: false }) pais?: string;
  @ApiProperty({ required: false }) moneda?: string;
  @ApiProperty({ required: false }) zona_horaria?: string;
  @ApiProperty({ required: false }) sector?: string;
  @ApiProperty({ required: false }) representante_legal?: string;
  @ApiProperty({ required: false }) horario_atencion?: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) logo_objeto_id?: string | null;
  @ApiProperty({ required: false, nullable: true, description: 'URL pública derivada del logo' }) logo_url?: string | null;
  @ApiProperty({ required: false, format: 'date-time' }) actualizado_en?: string;
}
class VendedorEmpresaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty() nombre: string;
  @ApiProperty() correo: string;
  @ApiProperty({ enum: ['vendedor'] }) rol: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) participacion_porcentaje: number;
}

@ApiTags('Empresas') @ApiBearerAuth() @Controller('api/v1/empresas')
export class EmpresasController {
  constructor(private readonly empresas: EmpresasService) {}
  @Post() @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear empresa y elegir modalidad logística' })
  @ApiCreatedResponse({ type: EmpresaResponseDto, description: 'Empresa creada con membresía del empresario' })
  crear(@Body() dto: CrearEmpresaRequestDto, @UsuarioActual() u: UsuarioContexto) { return this.empresas.crear(u.user_id, dto.nombre, dto.modalidad_logistica, dto.slug, dto); }
  @Get(':id') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Obtener una empresa por UUID o slug (solo miembros o administradores)' })
  @ApiParam({
    name: 'id',
    description: 'UUID de la empresa o su slug público actual/anterior. Un slug anterior responde 301.',
    example: 'distribuidora-piloto',
  })
  @ApiOkResponse({ type: EmpresaResponseDto, description: 'Empresa visible para el miembro o administrador global' })
  @ApiResponse({
    status: 301,
    description: 'El slug consultado es anterior; Location apunta al slug actual',
    headers: { Location: { description: 'Ruta con el slug actual', schema: { type: 'string' } } },
  })
  @ApiResponse({ status: 404, description: 'Empresa inexistente o no visible para el usuario' })
  async obtener(
    @Param('id') id: string,
    @UsuarioActual() u: UsuarioContexto,
    @Res({ passthrough: true }) res: Response,
  ) {
    const { empresa, redirigirA } = await this.empresas.resolverPara(
      u.user_id,
      id,
      [ROLES.SUPER_ADMIN, ROLES.ADMIN].includes(u.rol as never),
    );
    if (redirigirA) res.status(301).setHeader('Location', redirigirA);
    return empresa;
  }
  @Patch(':id/perfil') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Actualizar el perfil comercial/fiscal y el logo' })
  @ApiParam({ name: 'id', description: 'UUID de la empresa o su slug actual' })
  @ApiOkResponse({ type: EmpresaResponseDto, description: 'Perfil actualizado' })
  actualizarPerfil(
    @Param('id') id: string,
    @Body() dto: PerfilEmpresaRequestDto,
    @UsuarioActual() u: UsuarioContexto,
  ) {
    const esGlobal = [ROLES.SUPER_ADMIN, ROLES.ADMIN].includes(u.rol as never);
    return this.empresas.actualizarPerfil(u.user_id, id, dto, esGlobal);
  }
  @Patch(':id/slug') @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Cambiar el slug público; los anteriores redirigen 301' })
  @ApiParam({ name: 'id', description: 'UUID de la empresa o su slug actual/anterior' })
  @ApiOkResponse({ type: EmpresaResponseDto, description: 'Slug actualizado' })
  @ApiResponse({ status: 409, description: 'El slug ya está en uso (incluye slugs históricos)' })
  actualizarSlug(
    @Param('id') id: string,
    @Body() dto: ActualizarSlugRequestDto,
    @UsuarioActual() u: UsuarioContexto,
  ) {
    return this.empresas.actualizarSlug(u.user_id, id, dto.slug);
  }
  @Patch(':id/logistica') @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Cambiar la modalidad logística para envíos futuros' })
  @ApiOkResponse({ type: EmpresaResponseDto, description: 'Modalidad logística actualizada' })
  actualizarLogistica(@Param('id') id: string, @Body() dto: ActualizarModalidadLogisticaRequestDto, @UsuarioActual() u: UsuarioContexto) {
    return this.empresas.actualizarModalidad(u.user_id, id, dto.modalidad_logistica);
  }
  @Patch(':id/tipo') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Clasificar la empresa (comercial, logística o plataforma)' })
  @ApiParam({ name: 'id', description: 'UUID de la empresa' })
  @ApiOkResponse({ type: EmpresaResponseDto, description: 'Tipo de empresa actualizado' })
  actualizarTipo(@Param('id') id: string, @Body() dto: ActualizarTipoEmpresaRequestDto) {
    return this.empresas.actualizarTipo(id, dto.tipo_empresa);
  }
  @Post(':id/vendedores') @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear vendedor con participación predeterminada' })
  @ApiCreatedResponse({ type: VendedorEmpresaResponseDto, description: 'Vendedor creado y asociado a la empresa' })
  vendedor(@Param('id') id: string, @Body() dto: CrearVendedorRequestDto, @UsuarioActual() u: UsuarioContexto) { return this.empresas.crearVendedor(u.user_id, id, dto); }
}
