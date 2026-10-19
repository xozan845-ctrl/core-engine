import { Body, Controller, Get, Param, Post } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiProperty, ApiTags } from '@nestjs/swagger';
import { IsEmail, IsIn, IsInt, IsString, Length, Max, Min, MinLength } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { EmpresasService, ModalidadLogistica } from './empresas.service';

class CrearEmpresaRequestDto {
  @ApiProperty({ minLength: 2, maxLength: 160 }) @IsString() @Length(2, 160) nombre: string;
  @ApiProperty({ enum: ['plataforma', 'propia', 'hibrida'] }) @IsIn(['plataforma', 'propia', 'hibrida']) modalidad_logistica: ModalidadLogistica;
}
class CrearVendedorRequestDto {
  @ApiProperty() @IsString() @Length(2, 120) nombre: string;
  @ApiProperty() @IsEmail({ require_tld: false }) correo: string;
  @ApiProperty({ minLength: 8 }) @IsString() @MinLength(8) contrasena: string;
  @ApiProperty({ minimum: 0, maximum: 100 }) @IsInt() @Min(0) @Max(100) participacion_porcentaje: number;
}
export class EmpresaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty() nombre: string;
  @ApiProperty({ enum: ['plataforma', 'propia', 'hibrida'] }) modalidad_logistica: ModalidadLogistica;
  @ApiProperty() estado: string;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty() creado_en: string;
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
  crear(@Body() dto: CrearEmpresaRequestDto, @UsuarioActual() u: UsuarioContexto) { return this.empresas.crear(u.user_id, dto.nombre, dto.modalidad_logistica); }
  @Get(':id') @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR)
  @ApiOkResponse({ type: EmpresaResponseDto, description: 'Empresa visible para el miembro o administrador global' })
  obtener(@Param('id') id: string, @UsuarioActual() u: UsuarioContexto) { return this.empresas.obtenerPara(u.user_id, id, [ROLES.SUPER_ADMIN, ROLES.ADMIN].includes(u.rol as never)); }
  @Post(':id/vendedores') @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Crear vendedor con participación predeterminada' })
  @ApiCreatedResponse({ type: VendedorEmpresaResponseDto, description: 'Vendedor creado y asociado a la empresa' })
  vendedor(@Param('id') id: string, @Body() dto: CrearVendedorRequestDto, @UsuarioActual() u: UsuarioContexto) { return this.empresas.crearVendedor(u.user_id, id, dto); }
}
