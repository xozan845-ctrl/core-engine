import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
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

export class CrearProductoDto {
  @IsString()
  @Matches(/^[A-Za-z0-9\-]{2,32}$/, { message: 'SKU invalido (letras, numeros y guiones, 2-32)' })
  sku: string;

  @IsString()
  @Length(2, 200)
  nombre: string;

  @IsOptional()
  @IsString()
  descripcion?: string;

  @IsOptional()
  @IsString()
  categoria?: string;

  /** Formato decimal ("1000.00") en cordobas; se almacena en centavos (A02). */
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  precio_base: string;

  @IsInt()
  @Min(0)
  stock: number;
}

export class ActualizarProductoDto {
  @IsOptional()
  @IsString()
  @Length(2, 200)
  nombre?: string;

  @IsOptional()
  @IsString()
  descripcion?: string;

  @IsOptional()
  @IsString()
  categoria?: string;

  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  precio_base?: string;

  @IsOptional()
  @IsInt()
  @Min(0)
  stock?: number;

  @IsOptional()
  @IsString()
  motivo?: string;
}

export class ListarProductosDto {
  @IsOptional()
  @IsString()
  q?: string;

  @IsOptional()
  @IsString()
  estado?: string;

  @IsOptional()
  @IsString()
  categoria?: string;

  @IsOptional()
  @IsPositive()
  pagina?: number;

  @IsOptional()
  @IsPositive()
  @Max(100)
  limite?: number;
}

@Controller('api/v1/catalog')
export class ProductosController {
  constructor(private readonly productos: ProductosService) {}

  /** GET /api/v1/catalog/productos — listado publico con filtros y paginacion. */
  @Get('productos')
  async listar(@Query() query: ListarProductosDto): Promise<Pagina<Producto>> {
    return this.productos.listar(query as unknown as Record<string, unknown>);
  }

  /** GET /api/v1/catalog/productos/:id — detalle publico. */
  @Get('productos/:id')
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
  async crear(
    @Body() dto: CrearProductoDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    return this.productos.crear({ ...dto, precio_base: dto.precio_base });
  }

  /** PATCH /api/v1/catalog/productos/:id — edicion (admin; RN-08 historico). */
  @Patch('productos/:id')
  @Roles(ROLES.ADMIN)
  async actualizar(@Param('id') id: string, @Body() dto: ActualizarProductoDto): Promise<Producto> {
    return this.productos.actualizar(id, dto);
  }
}