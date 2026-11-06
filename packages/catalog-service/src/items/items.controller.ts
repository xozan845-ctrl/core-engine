import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiTags,
} from '@nestjs/swagger';
import { IsIn, IsObject, IsOptional, IsString, IsUUID } from 'class-validator';
import {
  CAPACIDADES,
  DomainError,
  NotFoundError,
  RequiereCapacidades,
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { Item, ItemsService, TipoItem } from './items.service';
import {
  EsquemaGuardadoResponseDto,
  ItemResponseDto,
  ItemSchemaResponseDto,
  PaginaItemsResponseDto,
} from './items.response.dto';

export class CrearAlquilerRequestDto {
  @ApiProperty({ example: 'ALQ-001' }) @IsString() codigo: string;
  @ApiProperty() @IsString() nombre: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() descripcion?: string;
  @ApiProperty({ required: false, default: 'general' }) @IsOptional() @IsString() categoria?: string;
  @ApiProperty({ example: '1500.00' }) @IsString() precio: string;
  @ApiProperty({ required: false, example: '500.00' }) @IsOptional() @IsString() costo?: string;
  @ApiProperty({ required: false, type: Object, additionalProperties: true }) @IsOptional() @IsObject() atributos?: Record<string, unknown>;
  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] }) @IsOptional() @IsIn(['activo', 'inactivo']) estado?: 'activo' | 'inactivo';
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() empresa_id?: string;
}

export class GuardarEsquemaRequestDto {
  @ApiProperty({ enum: ['producto', 'servicio', 'alquiler'] }) @IsIn(['producto', 'servicio', 'alquiler']) tipo: TipoItem;
  @ApiProperty() @IsString() categoria: string;
  @ApiProperty({ type: Object, additionalProperties: true }) @IsObject() esquema: Record<string, unknown>;
}

@ApiTags('Catálogo · Items')
@ApiBearerAuth()
@Controller('api/v1/catalog/items')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.CATALOGO_ITEMS)
export class ItemsController {
  constructor(private readonly items: ItemsService) {}

  @Get()
  @ApiOperation({ summary: 'Listar items unificados de una empresa' })
  @ApiOkResponse({ type: PaginaItemsResponseDto })
  async listar(@Query() query: Record<string, unknown>, @UsuarioActual() usuario: UsuarioContexto) {
    return this.items.listar({ ...query, empresa_id: this.empresaDeConsulta(usuario, query.empresa_id) });
  }

  @Post('alquileres')
  @ApiOperation({ summary: 'Crear un item de alquiler' })
  @ApiCreatedResponse({ type: ItemResponseDto })
  async crearAlquiler(@Body() dto: CrearAlquilerRequestDto, @UsuarioActual() usuario: UsuarioContexto) {
    return this.items.crearAlquiler(dto, this.empresaDeEscritura(usuario, dto.empresa_id));
  }

  @Post('atributos/esquemas')
  @ApiOperation({ summary: 'Registrar un JSON Schema de atributos por categoría' })
  @ApiOkResponse({ type: EsquemaGuardadoResponseDto, description: 'Esquema registrado' })
  async guardarEsquema(@Body() dto: GuardarEsquemaRequestDto, @UsuarioActual() usuario: UsuarioContexto) {
    await this.items.guardarEsquema(
      usuario.rol === ROLES.EMPRESARIO ? usuario.tenant_id ?? null : null,
      dto.tipo,
      dto.categoria,
      dto.esquema,
    );
    return { ok: true };
  }

  @Get('atributos/esquemas')
  @ApiOperation({ summary: 'Listar JSON Schemas de atributos' })
  @ApiOkResponse({ type: [ItemSchemaResponseDto], description: 'Esquemas activos' })
  async esquemas(@UsuarioActual() usuario: UsuarioContexto) {
    return this.items.esquemas(usuario.tenant_id ?? undefined);
  }

  @Get(':id')
  @ApiOperation({ summary: 'Consultar un item unificado' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: ItemResponseDto })
  async detalle(@Param('id') id: string, @Query('empresa_id') empresaId: string | undefined, @UsuarioActual() usuario: UsuarioContexto): Promise<Item> {
    const item = await this.items.encontrarPorId(id, this.empresaDeConsulta(usuario, empresaId));
    if (!item) throw new NotFoundError('Item', id);
    return item;
  }

  private empresaDeConsulta(usuario: UsuarioContexto, solicitada: unknown): string {
    if (usuario.rol === ROLES.EMPRESARIO) {
      if (!usuario.tenant_id) throw new DomainError('EMPRESA_REQUERIDA', 'No hay empresa activa.');
      if (typeof solicitada === 'string' && solicitada !== usuario.tenant_id) throw new DomainError('EMPRESA_NO_AUTORIZADA', 'No puedes consultar otra empresa.');
      return usuario.tenant_id;
    }
    if (typeof solicitada !== 'string' || !solicitada) throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id.');
    return solicitada;
  }

  private empresaDeEscritura(usuario: UsuarioContexto, solicitada?: string): string {
    if (usuario.rol === ROLES.EMPRESARIO) {
      if (!usuario.tenant_id) throw new DomainError('EMPRESA_REQUERIDA', 'No hay empresa activa.');
      return usuario.tenant_id;
    }
    if (!solicitada) throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id.');
    return solicitada;
  }
}
