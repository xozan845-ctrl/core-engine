import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  Param,
  Patch,
  Post,
  Query,
} from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiNoContentResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiResponse,
  ApiTags,
} from '@nestjs/swagger';
import {
  IsArray,
  IsBoolean,
  IsIn,
  IsInt,
  IsObject,
  IsOptional,
  IsPositive,
  IsString,
  IsUUID,
  Length,
  Matches,
  Max,
  Min,
  ValidateNested,
} from 'class-validator';
import { Type } from 'class-transformer';
import { ProductosService, Producto } from './productos.service';
import {
  PaginaProductosResponseDto,
  ProductoResponseDto,
} from './productos.response.dto';
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

export class ImagenProductoRequestDto {
  @ApiProperty({ description: 'Id del objeto confirmado en storage-service', format: 'uuid' })
  @IsUUID()
  objeto_id: string;

  @ApiProperty({ description: 'Orden de la imagen (0-2); si se omite se asigna el siguiente', required: false, minimum: 0, maximum: 2 })
  @IsOptional()
  @IsInt()
  @Min(0)
  @Max(2)
  orden?: number;

  @ApiProperty({ description: 'Marcar como imagen principal', required: false, default: false })
  @IsOptional()
  @IsBoolean()
  es_principal?: boolean;
}

export class VarianteProductoRequestDto {
  @ApiProperty({ description: 'SKU de la variante', pattern: '^[A-Za-z0-9-]{2,32}$', example: 'SKU-001-ROJO-M' })
  @IsString()
  @Matches(/^[A-Za-z0-9-]{2,32}$/, { message: 'SKU invalido (letras, numeros y guiones, 2-32)' })
  sku: string;

  @ApiProperty({ description: 'Nombre visible de la variante', required: false })
  @IsOptional()
  @IsString()
  @Length(2, 200)
  nombre?: string;

  @ApiProperty({ description: 'Atributos de la variante (color, talla, etc.)', required: false })
  @IsOptional()
  @IsObject()
  atributos?: Record<string, unknown>;

  @ApiProperty({ description: 'Precio base decimal (ej: 1000.00)', example: '1500.00' })
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  precio_base: string;

  @ApiProperty({ description: 'Stock inicial de la variante', minimum: 0, example: 5 })
  @IsInt()
  @Min(0)
  stock: number;

  @ApiProperty({ description: 'Imágenes de la variante (máx. 3)', type: () => [ImagenProductoRequestDto], required: false })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => ImagenProductoRequestDto)
  imagenes?: ImagenProductoRequestDto[];
}

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


  @ApiProperty({ description: 'Costo decimal en córdobas (ej: 150.00)', required: false, example: '150.00' })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Costo invalido (ej: 150.00)' })
  costo?: string;

  @ApiProperty({ description: 'Marca', required: false })
  @IsOptional()
  @IsString()
  marca?: string;

  @ApiProperty({ description: 'Código de barras único por empresa', required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ description: 'Unidad de venta', required: false, example: 'Unidad (u)' })
  @IsOptional()
  @IsString()
  unidad?: string;

  @ApiProperty({ description: 'Permite cantidades fraccionarias', required: false, default: false })
  @IsOptional()
  @IsBoolean()
  permite_fracciones?: boolean;

  @ApiProperty({ description: 'Unidad de compra', required: false, example: 'Igual que la de venta' })
  @IsOptional()
  @IsString()
  unidad_compra?: string;

  @ApiProperty({ description: 'Contenido por unidad', required: false, minimum: 0, default: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  contenido_cantidad?: number;

  @ApiProperty({ description: 'Unidad del contenido', required: false, example: 'Sin contenido' })
  @IsOptional()
  @IsString()
  contenido_unidad?: string;

  @ApiProperty({ description: 'Tipo de precio', required: false, enum: ['fijo', 'escala'] })
  @IsOptional()
  @IsIn(['fijo', 'escala'])
  tipo_precio?: 'fijo' | 'escala';

  @ApiProperty({ description: 'Tipo de costo', required: false, enum: ['fijo', 'escala'] })
  @IsOptional()
  @IsIn(['fijo', 'escala'])
  tipo_costo?: 'fijo' | 'escala';

  @ApiProperty({ description: 'Stock mínimo para alertas', required: false, minimum: 0, default: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_minimo?: number;

  @ApiProperty({ description: 'Stock máximo', required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_maximo?: number | null;

  @ApiProperty({ description: 'Garantía en meses', required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  garantia_meses?: number | null;

  @ApiProperty({ description: 'Es materia prima', required: false, default: false })
  @IsOptional()
  @IsBoolean()
  es_materia_prima?: boolean;

  @ApiProperty({ description: 'Ubicación física', required: false })
  @IsOptional()
  @IsString()
  ubicacion?: string;

  @ApiProperty({ description: 'Estado comercial', required: false, enum: ['activo', 'inactivo'], default: 'activo' })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado_operativo?: 'activo' | 'inactivo';

  @ApiProperty({ description: 'Atributos del producto (si aplica)', required: false })
  @IsOptional()
  @IsObject()
  atributos?: Record<string, unknown>;

  @ApiProperty({ description: 'Imágenes del producto (máx. 3)', type: () => [ImagenProductoRequestDto], required: false })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => ImagenProductoRequestDto)
  imagenes?: ImagenProductoRequestDto[];

  @ApiProperty({ description: 'Variantes vendibles del producto (máx. 50)', type: () => [VarianteProductoRequestDto], required: false })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => VarianteProductoRequestDto)
  variantes?: VarianteProductoRequestDto[];
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


  @ApiProperty({ description: 'Costo decimal en córdobas (ej: 150.00)', required: false, example: '150.00' })
  @IsOptional()
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Costo invalido (ej: 150.00)' })
  costo?: string;

  @ApiProperty({ description: 'Marca', required: false })
  @IsOptional()
  @IsString()
  marca?: string;

  @ApiProperty({ description: 'Código de barras único por empresa', required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ description: 'Unidad de venta', required: false, example: 'Unidad (u)' })
  @IsOptional()
  @IsString()
  unidad?: string;

  @ApiProperty({ description: 'Permite cantidades fraccionarias', required: false })
  @IsOptional()
  @IsBoolean()
  permite_fracciones?: boolean;

  @ApiProperty({ description: 'Unidad de compra', required: false, example: 'Igual que la de venta' })
  @IsOptional()
  @IsString()
  unidad_compra?: string;

  @ApiProperty({ description: 'Contenido por unidad', required: false, minimum: 0, default: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  contenido_cantidad?: number;

  @ApiProperty({ description: 'Unidad del contenido', required: false, example: 'Sin contenido' })
  @IsOptional()
  @IsString()
  contenido_unidad?: string;

  @ApiProperty({ description: 'Tipo de precio', required: false, enum: ['fijo', 'escala'] })
  @IsOptional()
  @IsIn(['fijo', 'escala'])
  tipo_precio?: 'fijo' | 'escala';

  @ApiProperty({ description: 'Tipo de costo', required: false, enum: ['fijo', 'escala'] })
  @IsOptional()
  @IsIn(['fijo', 'escala'])
  tipo_costo?: 'fijo' | 'escala';

  @ApiProperty({ description: 'Stock mínimo para alertas', required: false, minimum: 0, default: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_minimo?: number;

  @ApiProperty({ description: 'Stock máximo', required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_maximo?: number | null;

  @ApiProperty({ description: 'Garantía en meses', required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  garantia_meses?: number | null;

  @ApiProperty({ description: 'Es materia prima', required: false })
  @IsOptional()
  @IsBoolean()
  es_materia_prima?: boolean;

  @ApiProperty({ description: 'Ubicación física', required: false })
  @IsOptional()
  @IsString()
  ubicacion?: string;

  @ApiProperty({ description: 'Estado comercial', required: false, enum: ['activo', 'inactivo'], default: 'activo' })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado_operativo?: 'activo' | 'inactivo';

  @ApiProperty({ description: 'Atributos de la variante', required: false })
  @IsOptional()
  @IsObject()
  atributos?: Record<string, unknown>;

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
    return this.productos.listar({
      ...query,
      empresa_id: empresaId,
      incluir_inactivos: this.puedeVerInactivos(usuario, empresaId),
    });
  }

  /** GET /api/v1/catalog/productos/:id — detalle publico acotado a la empresa. */
  @Get('productos/:id')
  @ApiOperation({ summary: 'Detalle de producto de una empresa (público, incluye variantes)' })
  @ApiParam({ name: 'id', description: 'Id del producto' })
  @ApiOkResponse({ type: ProductoResponseDto, description: 'Producto encontrado' })
  @ApiResponse({ status: 404, description: 'Producto inexistente o de otra empresa (NO_ENCONTRADO)' })
  async detalle(
    @Param('id') id: string,
    @Query() query: DetalleProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto | null,
  ): Promise<Producto> {
    const empresaId = this.empresaDeConsulta(usuario, query.empresa_id);
    const producto = await this.productos.encontrarPorId(
      id,
      empresaId,
      this.puedeVerInactivos(usuario, empresaId),
    );
    if (!producto) {
      throw new NotFoundError('Producto', id);
    }
    return producto;
  }

  /** POST /api/v1/catalog/productos — alta de producto (admin, TC-01). */
  @Post('productos')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Crear producto con variantes e imágenes (admin, TC-01)' })
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

  /** POST /api/v1/catalog/productos/:id/variantes — agrega una variante. */
  @Post('productos/:id/variantes')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Agregar una variante al producto (ADR-28)' })
  @ApiParam({ name: 'id', description: 'Id del producto padre' })
  @ApiCreatedResponse({ type: ProductoResponseDto, description: 'Variante creada' })
  async crearVariante(
    @Param('id') id: string,
    @Body() dto: VarianteProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const empresaId = await this.empresaDeProducto(id, usuario);
    return this.productos.crearVariante(id, dto, empresaId);
  }

  /** PATCH /api/v1/catalog/productos/:id/variantes/:varianteId — edita la variante. */
  @Patch('productos/:id/variantes/:varianteId')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Editar una variante (precio/stock/atributos)' })
  @ApiParam({ name: 'id', description: 'Id del producto padre' })
  @ApiParam({ name: 'varianteId', description: 'Id de la variante' })
  @ApiOkResponse({ type: ProductoResponseDto, description: 'Variante actualizada' })
  async actualizarVariante(
    @Param('id') id: string,
    @Param('varianteId') varianteId: string,
    @Body() dto: ActualizarProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const empresaId = await this.empresaDeProducto(id, usuario);
    return this.productos.actualizarVariante(id, varianteId, dto, empresaId);
  }

  /** DELETE /api/v1/catalog/productos/:id/variantes/:varianteId — elimina la variante. */
  @Delete('productos/:id/variantes/:varianteId')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Eliminar una variante' })
  @ApiParam({ name: 'id', description: 'Id del producto padre' })
  @ApiParam({ name: 'varianteId', description: 'Id de la variante' })
  @ApiNoContentResponse({ description: 'Variante eliminada' })
  @HttpCode(204)
  async eliminarVariante(
    @Param('id') id: string,
    @Param('varianteId') varianteId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    const empresaId = await this.empresaDeProducto(id, usuario);
    await this.productos.eliminarVariante(id, varianteId, empresaId);
  }

  /** POST /api/v1/catalog/productos/:id/imagenes — agrega una imagen (máx. 3). */
  @Post('productos/:id/imagenes')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Agregar una imagen a un producto o variante (máx. 3)' })
  @ApiParam({ name: 'id', description: 'Id del producto o variante' })
  @ApiCreatedResponse({ type: ProductoResponseDto, description: 'Imagen agregada' })
  async agregarImagen(
    @Param('id') id: string,
    @Body() dto: ImagenProductoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const empresaId = await this.empresaDeProducto(id, usuario);
    return this.productos.agregarImagen(id, dto, empresaId);
  }

  /** DELETE /api/v1/catalog/productos/:id/imagenes/:imagenId — elimina una imagen. */
  @Delete('productos/:id/imagenes/:imagenId')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiBearerAuth()
  @ApiOperation({ summary: 'Eliminar una imagen de un producto o variante' })
  @ApiParam({ name: 'id', description: 'Id del producto o variante' })
  @ApiParam({ name: 'imagenId', description: 'Id de la relación producto-imagen' })
  @ApiNoContentResponse({ description: 'Imagen eliminada' })
  @HttpCode(204)
  async eliminarImagen(
    @Param('id') id: string,
    @Param('imagenId') imagenId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    const empresaId = await this.empresaDeProducto(id, usuario);
    await this.productos.eliminarImagen(id, imagenId, empresaId);
  }

  /** ADR-27: el catálogo público exige empresa; la sesión aporta el tenant. */
  private empresaDeConsulta(usuario: UsuarioContexto | null, solicitada?: string): string {
    if (usuario?.rol === ROLES.EMPRESARIO) {
      if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
      if (solicitada && solicitada !== usuario.tenant_id) {
        throw new DomainError(
          'EMPRESA_NO_AUTORIZADA',
          'El empresario solo puede consultar el catálogo de su empresa.',
        );
      }
      return usuario.tenant_id;
    }
    const empresaId = solicitada ?? usuario?.tenant_id;
    if (!empresaId) {
      throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id o autentícate con una empresa.');
    }
    if (empresaId === EMPRESA_LEGACY_ID) {
      throw new DomainError('EMPRESA_NO_PUBLICA', 'La empresa comodín del catálogo legado no publica productos.');
    }
    return empresaId;
  }

  /** Solo el dueño del catálogo o un admin pueden ver productos inactivos. */
  private puedeVerInactivos(usuario: UsuarioContexto | null, empresaId: string): boolean {
    if (!usuario) return false;
    if (usuario.rol === ROLES.SUPER_ADMIN || usuario.rol === ROLES.ADMIN) return true;
    return usuario.rol === ROLES.EMPRESARIO && usuario.tenant_id === empresaId;
  }

  /** Empresario: su tenant. Admin: la empresa del producto. */
  private async empresaDeProducto(id: string, usuario: UsuarioContexto): Promise<string> {
    if (usuario.rol !== ROLES.SUPER_ADMIN && usuario.rol !== ROLES.ADMIN) {
      if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
      return usuario.tenant_id;
    }
    const producto = await this.productos.encontrarPorId(id);
    if (!producto) throw new NotFoundError('Producto', id);
    return producto.empresa_id;
  }
}
