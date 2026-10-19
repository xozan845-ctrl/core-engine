import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  Param,
  Post,
  Query,
} from '@nestjs/common';
import { ApiBearerAuth, ApiOperation, ApiParam, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import {
  IsInt,
  IsObject,
  IsOptional,
  IsPositive,
  IsString,
  Length,
  Matches,
  Max,
} from 'class-validator';
import { UsuarioActual, UsuarioContexto } from '@core/shared';
import { requerirTenant } from '../tenant';
import { ObjetoStorage, ObjetosService, SolicitudSubida, UrlDescarga } from './objetos.service';

export class SolicitarSubidaRequestDto {
  @ApiProperty({ description: 'Nombre del archivo (sin rutas)', example: 'foto-producto.jpg' })
  @IsString()
  @Length(1, 200)
  nombre: string;

  @ApiProperty({ description: 'Content-Type del archivo (whitelist)', example: 'image/jpeg' })
  @IsString()
  @Length(3, 120)
  content_type: string;

  @ApiProperty({ description: 'Tamano en bytes que el cliente subira', example: 204800 })
  @IsInt()
  @IsPositive()
  tamano_bytes: number;

  @ApiProperty({ description: 'Carpeta logica dentro del bucket', required: false, example: 'productos' })
  @IsOptional()
  @IsString()
  @Matches(/^[A-Za-z0-9._/-]{1,40}$/, { message: 'Carpeta invalida (letras, numeros, . _ -)' })
  carpeta?: string;

  @ApiProperty({ description: 'Descripcion libre', required: false })
  @IsOptional()
  @IsString()
  @Length(0, 500)
  descripcion?: string;

  @ApiProperty({ description: 'Etiquetas clave-valor (max 20)', required: false })
  @IsOptional()
  @IsObject()
  etiquetas?: Record<string, string>;
}

export class ListarObjetosRequestDto {
  @ApiProperty({ description: 'Busqueda por nombre', required: false })
  @IsOptional()
  @IsString()
  q?: string;

  @ApiProperty({ description: 'Filtrar por carpeta', required: false })
  @IsOptional()
  @IsString()
  carpeta?: string;

  @ApiProperty({ description: 'Filtrar por estado', required: false, enum: ['pendiente', 'activo', 'eliminado'] })
  @IsOptional()
  @IsString()
  estado?: string;

  @ApiProperty({ description: 'Pagina (1-based)', required: false, minimum: 1 })
  @IsOptional()
  @IsPositive()
  pagina?: number;

  @ApiProperty({ description: 'Tamano de pagina (max 100)', required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsPositive()
  @Max(100)
  limite?: number;
}

@ApiTags('Almacenamiento')
@ApiBearerAuth()
@Controller('api/v1/storage')
export class ObjetosController {
  constructor(private readonly objetos: ObjetosService) {}

  @Post('subidas')
  @ApiOperation({
    summary: 'Solicitar URL prefirmada de subida (PUT directo a MinIO)',
    description:
      'Valida tipo, tamano y cuota del tenant; registra el objeto en estado pendiente y devuelve la URL prefirmada.',
  })
  @ApiResponse({ status: 400, description: 'Tipo o tamano invalido (VALIDACION)' })
  @ApiResponse({ status: 409, description: 'Cuota excedida (CUOTA_EXCEDIDA)' })
  async solicitarSubida(
    @Body() dto: SolicitarSubidaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<SolicitudSubida> {
    return this.objetos.solicitarSubida(requerirTenant(usuario), usuario?.user_id, dto);
  }

  @Post('objetos/:id/confirmar')
  @ApiOperation({ summary: 'Confirmar la subida y activar el objeto (contabiliza cuota)' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  @HttpCode(200)
  async confirmar(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<ObjetoStorage> {
    return this.objetos.confirmar(requerirTenant(usuario), id);
  }

  @Get('objetos')
  @ApiOperation({ summary: 'Listar objetos del tenant (paginado, sin eliminados por defecto)' })
  async listar(
    @Query() query: ListarObjetosRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.objetos.listar(requerirTenant(usuario), query);
  }

  @Get('objetos/:id')
  @ApiOperation({ summary: 'Detalle (metadata) de un objeto del tenant' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  async detalle(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<ObjetoStorage> {
    return this.objetos.obtener(requerirTenant(usuario), id);
  }

  @Get('objetos/:id/descarga')
  @ApiOperation({ summary: 'URL prefirmada de descarga (GET) con validez limitada' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  async descarga(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<UrlDescarga> {
    return this.objetos.urlDescarga(requerirTenant(usuario), id);
  }

  @Delete('objetos/:id')
  @ApiOperation({ summary: 'Eliminar un objeto (baja logica + borrado en MinIO)' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  @HttpCode(204)
  async eliminar(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    await this.objetos.eliminar(requerirTenant(usuario), id, usuario);
  }
}
