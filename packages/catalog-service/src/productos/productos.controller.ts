import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import {
  IsInt,
  IsOptional,
  IsPositive,
  IsString,
  Length,
  Matches,
  Max,
  Min,
} from 'class-validator';
import { ProductosService, Producto } from './productos.service';
import { Pagina, Roles, UsuarioContexto, UsuarioActual, ROLES, NotFoundError } from '@core/shared';

export class CrearProductoRequestDto {
  @ApiProperty({ description: 'SKU del producto', pattern: '^[A-Za-z0-9-]{2,32}$', example: 'SKU-001' })
  @IsString()
  @Matches(/^[A-Za-z0-9-]{2,32}$/, { message: 'SKU invalido (letras, numeros y guiones, 2-32)' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto', minLength: 2, maxLength: 200 })
  @IsString()
  @Length(2, 200)
  nombre: string;

  @ApiProperty({ description: 'Descripción', required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ description: 'Categoría', required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  /** Formato decimal ("1000.00") en cordobas; se almacena en centavos (A02). */
  @ApiProperty({ description: 'Precio base decimal (ej: 1000.00) en córdobas; se guarda en centavos', example: '1000.00' })
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  precio_base: string;

  @ApiProperty({ description: 'Stock inicial', minimum: 0, example: 10 })
  @IsInt()
  @Min(0)
  stock: number;
}

export class ActualizarProductoRequestDto {
  @ApiProperty({ description: 'Nombre del producto', required: false, minLength: 2, maxLength: 200 })
  @IsOptional()
  @IsString()
  @Length(2, 200)
  nombre?: string;

  @ApiProperty({ description: 'Descripción', required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ description: 'Categoría', required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  @ApiProperty({ description: 'Precio base decimal (ej: 1000.00)', required: false })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  precio_base?: string;

  @ApiProperty({ description: 'Stock', required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock?: number;

  @ApiProperty({ description: 'Motivo del cambio (RN-08 histórico)', required: false })
  @IsOptional()
  @IsString()
  motivo?: string;
}

export class ListarProductosRequestDto {
  @ApiProperty({ description: 'Búsqueda por texto', required: false })
  @IsOptional()
  @IsString()
  q?: string;

  @ApiProperty({ description: 'Filtrar por estado', required: false })
  @IsOptional()
  @IsString()
  estado?: string;

  @ApiProperty({ description: 'Filtrar por categoría', required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  @ApiProperty({ description: 'Página (1-based)', required: false, minimum: 1 })
  @IsOptional()
  @IsPositive()
  pagina?: number;

  @ApiProperty({ description: 'Tamaño de página (máx 100)', required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsPositive()
  @Max(100)
  limite?: number;
}

@ApiTags('Catálogo')
@Controller('api/v1/catalog')
export class ProductosController {
  constructor(private readonly productos: ProductosService) {}

  /** GET /api/v1/catalog/productos — listado publico con filtros y paginacion. */
  @Get('productos')
  @ApiOperation({ summary: 'Listar productos (público, filtros y paginación)' })
  async listar(@Query() query: ListarProductosRequestDto): Promise<Pagina<Producto>> {
    return this.productos.listar(query as unknown as Record<string, unknown>);
  }

  /** GET /api/v1/catalog/productos/:id — detalle publico. */
  @Get('productos/:id')
  @ApiOperation({ summary: 'Detalle de producto (público)' })
  @ApiParam({ name: 'id', description: 'Id del producto' })
  @ApiResponse({ status: 404, description: 'Producto inexistente (NO_ENCONTRADO)' })
  async detalle(@Param('id') id: string): Promise<Producto> {
    const producto = await this.productos.encontrarPorId(id);
    if (!producto) {
      throw new NotFoundError('Producto', id);
    }
    return producto;
  }

  /** POST /api/v1/catalog/productos — alta de producto (admin, TC-01). */
  @Post('productos')
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Crear producto (admin, TC-01)' })
  async crear(
    @Body() dto: CrearProductoRequestDto,
    @UsuarioActual() _usuario: UsuarioContexto,
  ): Promise<Producto> {
    return this.productos.crear({ ...dto, precio_base: dto.precio_base });
  }

  /** PATCH /api/v1/catalog/productos/:id — edicion (admin; RN-08 historico). */
  @Patch('productos/:id')
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Editar producto (admin; RN-08 histórico)' })
  @ApiParam({ name: 'id', description: 'Id del producto' })
  async actualizar(@Param('id') id: string, @Body() dto: ActualizarProductoRequestDto): Promise<Producto> {
    return this.productos.actualizar(id, dto);
  }
}
