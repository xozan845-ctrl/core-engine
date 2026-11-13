import { Body, Controller, Get, Param, Patch, Post, Put, Query } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiQuery,
  ApiTags,
  PartialType,
} from '@nestjs/swagger';
import { IsBoolean, IsIn, IsInt, IsOptional, IsString, IsUUID, Matches, Min } from 'class-validator';
import {
  CAPACIDADES,
  DomainError,
  RequiereCapacidades,
  ROLES,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import {
  CrearUbicacionInput,
  StockItemUbicaciones,
  StockUbicacion,
  TIPOS_UBICACION,
  TipoUbicacion,
  Ubicacion,
  UbicacionesService,
} from './ubicaciones.service';

export class CrearUbicacionInventarioRequestDto {
  @ApiProperty({ example: 'bodega-central' })
  @Matches(/^[a-z][a-z0-9_-]{1,39}$/)
  codigo: string;

  @ApiProperty({ example: 'Bodega central' })
  @IsString()
  nombre: string;

  @ApiProperty({ required: false, enum: TIPOS_UBICACION, default: 'bodega' })
  @IsOptional()
  @IsIn(TIPOS_UBICACION)
  tipo?: TipoUbicacion;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  direccion?: string;

  @ApiProperty({ required: false, default: false })
  @IsOptional()
  @IsBoolean()
  predeterminada?: boolean;
}

export class ActualizarUbicacionInventarioRequestDto extends PartialType(CrearUbicacionInventarioRequestDto) {
  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  activo?: boolean;
}

export class AjustarStockUbicacionInventarioRequestDto {
  @ApiProperty({ minimum: 0 })
  @IsInt()
  @Min(0)
  stock: number;
}

export class TrasladarStockInventarioRequestDto {
  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  item_id: string;

  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  origen_id: string;

  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  destino_id: string;

  @ApiProperty({ minimum: 1 })
  @IsInt()
  @Min(1)
  cantidad: number;
}

export class UbicacionInventarioResponseDto implements Ubicacion {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty({ enum: TIPOS_UBICACION }) tipo: TipoUbicacion;
  @ApiProperty() direccion: string;
  @ApiProperty() predeterminada: boolean;
  @ApiProperty() activo: boolean;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class StockUbicacionInventarioResponseDto implements StockUbicacion {
  @ApiProperty({ format: 'uuid' }) ubicacion_id: string;
  @ApiProperty() ubicacion_codigo: string;
  @ApiProperty() ubicacion_nombre: string;
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty() sku: string;
  @ApiProperty() nombre: string;
  @ApiProperty() stock: number;
  @ApiProperty() stock_total: number;
}

export class StockItemUbicacionesInventarioResponseDto implements StockItemUbicaciones {
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty() sku: string;
  @ApiProperty() nombre: string;
  @ApiProperty() stock_total: number;
  @ApiProperty() total_ubicaciones: number;
  @ApiProperty({
    type: 'array',
    items: {
      type: 'object',
      properties: {
        ubicacion_id: { type: 'string', format: 'uuid' },
        codigo: { type: 'string' },
        nombre: { type: 'string' },
        stock: { type: 'number' },
      },
    },
  })
  ubicaciones: StockItemUbicaciones['ubicaciones'];
}

export class TrasladoInventarioResponseDto {
  @ApiProperty({ type: () => StockUbicacionInventarioResponseDto })
  origen: StockUbicacion;

  @ApiProperty({ type: () => StockUbicacionInventarioResponseDto })
  destino: StockUbicacion;
}

@ApiTags('Catálogo · Ubicaciones')
@ApiBearerAuth()
@Controller('api/v2/admin/ubicaciones')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.INVENTARIO_STOCK)
export class UbicacionesController {
  constructor(private readonly ubicaciones: UbicacionesService) {}

  @Get()
  @ApiOperation({ summary: 'Listar bodegas y sucursales de la empresa' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [UbicacionInventarioResponseDto], description: 'Ubicaciones' })
  listar(
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.ubicaciones.listar(this.empresaDe(usuario, empresaId));
  }

  @Post()
  @ApiOperation({ summary: 'Crear una bodega o sucursal' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: UbicacionInventarioResponseDto, description: 'Ubicacion creada' })
  crear(
    @Body() dto: CrearUbicacionInventarioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.ubicaciones.crear(this.empresaDe(usuario, empresaId), dto as CrearUbicacionInput);
  }

  @Patch(':id')
  @ApiOperation({ summary: 'Actualizar una ubicacion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: UbicacionInventarioResponseDto, description: 'Ubicacion actualizada' })
  actualizar(
    @Param('id') ubicacionId: string,
    @Body() dto: ActualizarUbicacionInventarioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.ubicaciones.actualizar(this.empresaDe(usuario, empresaId), ubicacionId, dto);
  }

  @Post('transferencias')
  @ApiOperation({ summary: 'Trasladar stock entre dos ubicaciones' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: TrasladoInventarioResponseDto, description: 'Stock trasladado' })
  trasladar(
    @Body() dto: TrasladarStockInventarioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.ubicaciones.trasladar(this.empresaDe(usuario, empresaId), dto);
  }

  @Get('items/:itemId/stock')
  @ApiOperation({ summary: 'Consultar el desglose de stock de un item' })
  @ApiParam({ name: 'itemId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: StockItemUbicacionesInventarioResponseDto, description: 'Stock por ubicacion' })
  stockItem(
    @Param('itemId') itemId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.ubicaciones.stockDeItem(this.empresaDe(usuario, empresaId), itemId);
  }

  @Get(':id/stock')
  @ApiOperation({ summary: 'Listar el stock de una ubicacion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [StockUbicacionInventarioResponseDto], description: 'Stock de la ubicacion' })
  listarStock(
    @Param('id') ubicacionId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.ubicaciones.listarStock(this.empresaDe(usuario, empresaId), ubicacionId);
  }

  @Put(':id/stock/:itemId')
  @ApiOperation({ summary: 'Fijar el stock de un item en una ubicacion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiParam({ name: 'itemId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: StockUbicacionInventarioResponseDto, description: 'Stock ajustado' })
  ajustarStock(
    @Param('id') ubicacionId: string,
    @Param('itemId') itemId: string,
    @Body() dto: AjustarStockUbicacionInventarioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.ubicaciones.ajustarStock(
      this.empresaDe(usuario, empresaId),
      ubicacionId,
      itemId,
      dto.stock,
    );
  }

  private empresaDe(usuario: UsuarioContexto, solicitada?: string): string {
    if (usuario.rol === ROLES.EMPRESARIO || usuario.rol_empresa_id) {
      if (!usuario.tenant_id) throw new DomainError('EMPRESA_REQUERIDA', 'No hay empresa activa.');
      if (solicitada && solicitada !== usuario.tenant_id) {
        throw new DomainError('EMPRESA_NO_AUTORIZADA', 'No puedes operar otra empresa.');
      }
      return usuario.tenant_id;
    }
    if (!solicitada) throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id.');
    return solicitada;
  }
}
