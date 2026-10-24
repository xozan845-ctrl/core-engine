import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import {
  IsInt,
  IsOptional,
  IsPositive,
  IsString,
  Length,
  Matches,
  Max,
  Min,
  IsUUID,
} from 'class-validator';
import { ProductosService, Producto } from './productos.service';
import { PaginaProductosResponseDto, ProductoResponseDto } from './productos.response.dto';
import {
  DomainError,
  EMPRESA_LEGACY_ID,
  Pagina,
  Roles,
  UsuarioContexto,
  UsuarioActual,
  ROLES,
  NotFoundError,
} from '@core/shared';

export class CrearProductoRequestDto {
  @ApiProperty({ description: 'SKU del producto', pattern: '^[A-Za-z0-9-]{2,32}$', example: 'SKU-001' })
  @IsString()
  @Matches(/^[A-Za-z0-9-]{2,32}$/, { message: 'SKU invalido (letras, numeros y guiones, 2-32)' })
  sku: string;

  @ApiProperty({
    description: 'Empresa destino. Obligatoria para super_admin/admin; el empresario usa su empresa activa.',
    format: 'uuid',
    required: false,
  })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

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
  @ApiProperty({
    description: 'Empresa cuyo catálogo se consulta (ADR-27). Obligatoria para lecturas anónimas; con sesión se usa la empresa del token.',
    format: 'uuid',
    required: false,
  })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;

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

export class DetalleProductoRequestDto {
  @ApiProperty({
    description: 'Empresa a la que debe pertenecer el producto (ADR-27). Obligatoria para lecturas anónimas.',
    format: 'uuid',
    required: false,
  })
  @IsOptional()
  @IsUUID()
  empresa_id?: string;
}

@ApiTags('Catálogo')
@Controller('api/v1/catalog')
export class ProductosController {
  constructor(private readonly productos: ProductosService) {}

  /** GET /api/v1/catalog/productos — listado publico de UNA empresa (ADR-27). */
  @Get('productos')
  @ApiOperation({ summary: 'Listar productos de una empresa (público, filtros y paginación)' })
  @ApiOkResponse({ type: PaginaProductosResponseDto, description: 'Página de productos' })
  async listar(
    @Query() query: ListarProductosRequestDto,
    @UsuarioActual() usuario: UsuarioContexto | null,
  ): Promise<Pagina<Producto>> {
    const empresaId = this.empresaDeConsulta(usuario, query.empresa_id);
    return this.productos.listar({ ...query, empresa_id: empresaId });
  }

  /** GET /api/v1/catalog/productos/:id — detalle publico acotado a la empresa. */
  @Get('productos/:id')
  @ApiOperation({ summary: 'Detalle de producto de una empresa (público)' })
  @ApiParam({ name: 'id', description: 'Id del producto' })
  @ApiOkResponse({ type: ProductoResponseDto, description: 'Producto encontrado' })
  @ApiResponse({ status: 404, description: 'Producto inexistente o de otra empresa (NO_ENCONTRADO)' })
  async detalle(
    @Param('id') id: string,
    @Query() query: DetalleProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto | null,
  ): Promise<Producto> {
    const empresaId = this.empresaDeConsulta(usuario, query.empresa_id);
    const producto = await this.productos.encontrarPorId(id, empresaId);
    if (!producto) {
      throw new NotFoundError('Producto', id);
    }
    return producto;
  }

  /** POST /api/v1/catalog/productos — alta de producto (admin, TC-01). */
  @Post('productos')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Crear producto (admin, TC-01)' })
  @ApiCreatedResponse({ type: ProductoResponseDto, description: 'Producto creado' })
  async crear(
    @Body() dto: CrearProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const esGlobal = usuario.rol === ROLES.SUPER_ADMIN || usuario.rol === ROLES.ADMIN;
    const empresaId = esGlobal ? dto.empresa_id : usuario.tenant_id;
    if (!empresaId) throw new NotFoundError('Empresa', 'activa');
    return this.productos.crear({ ...dto, empresa_id: empresaId, precio_base: dto.precio_base });
  }

  /** PATCH /api/v1/catalog/productos/:id — edicion (admin; RN-08 historico). */
  @Patch('productos/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Editar producto (admin; RN-08 histórico)' })
  @ApiParam({ name: 'id', description: 'Id del producto' })
  @ApiOkResponse({ type: ProductoResponseDto, description: 'Producto actualizado' })
  async actualizar(
    @Param('id') id: string,
    @Body() dto: ActualizarProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const empresaId = usuario.rol === ROLES.EMPRESARIO ? usuario.tenant_id : undefined;
    if (usuario.rol === ROLES.EMPRESARIO && !empresaId) throw new NotFoundError('Empresa', 'activa');
    return this.productos.actualizar(id, dto, empresaId);
  }

  /** ADR-27: el catálogo público exige empresa; la sesión aporta el tenant. */
  private empresaDeConsulta(usuario: UsuarioContexto | null, solicitada?: string): string {
    const empresaId = solicitada ?? usuario?.tenant_id;
    if (!empresaId) {
      throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id o autentícate con una empresa.');
    }
    if (empresaId === EMPRESA_LEGACY_ID) {
      throw new DomainError('EMPRESA_NO_PUBLICA', 'La empresa comodín del catálogo legado no publica productos.');
    }
    return empresaId;
  }
}
