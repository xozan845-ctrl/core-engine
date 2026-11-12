import {
  Body,
  Controller,
  Delete,
  HttpCode,
  Param,
  Patch,
  Post,
} from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiNoContentResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiTags,
} from '@nestjs/swagger';
import {
  IsArray,
  IsBoolean,
  IsIn,
  IsInt,
  IsObject,
  IsOptional,
  IsString,
  IsUUID,
  Matches,
  Max,
  Min,
  ValidateNested,
} from 'class-validator';
import { Type } from 'class-transformer';
import {
  CAPACIDADES,
  DomainError,
  NotFoundError,
  Permisos,
  RequiereCapacidades,
  ROLES,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { ItemsService } from './items.service';
import { ProductosService, Producto } from '../productos/productos.service';
import { ServiciosService, Servicio } from '../servicios/servicios.service';
import { ItemResponseDto } from './items.response.dto';

export class ImagenItemRequestDto {
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

export class VarianteItemRequestDto {
  @ApiProperty({ description: 'Código/SKU de la variante', pattern: '^[A-Za-z0-9-]{2,32}$', example: 'SKU-001-ROJO-M' })
  @IsString()
  @Matches(/^[A-Za-z0-9-]{2,32}$/, { message: 'SKU invalido (letras, numeros y guiones, 2-32)' })
  sku: string;

  @ApiProperty({ description: 'Nombre visible de la variante', required: false })
  @IsOptional()
  @IsString()
  nombre?: string;

  @ApiProperty({ description: 'Atributos de la variante (color, talla, etc.)', required: false })
  @IsOptional()
  @IsObject()
  atributos?: Record<string, unknown>;

  @ApiProperty({ description: 'Precio decimal (ej: 1000.00)', example: '1500.00' })
  @IsString()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  precio_base: string;

  @ApiProperty({ description: 'Stock inicial de la variante', minimum: 0, example: 5 })
  @IsInt()
  @Min(0)
  stock: number;

  @ApiProperty({ description: 'Imágenes de la variante (máx. 3)', type: () => [ImagenItemRequestDto], required: false })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => ImagenItemRequestDto)
  imagenes?: ImagenItemRequestDto[];
}

export class ActualizarItemCatalogoRequestDto {
  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  nombre?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  categoria?: string;

  /** Precio decimal; en productos se mapea a precio_base. */
  @ApiProperty({ required: false, example: '1000.00' })
  @IsOptional()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  @IsString()
  precio?: string;

  @ApiProperty({ required: false, example: '1000.00' })
  @IsOptional()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Precio invalido (ej: 1000.00)' })
  @IsString()
  precio_base?: string;

  @ApiProperty({ required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock?: number;

  @ApiProperty({ required: false, example: '150.00' })
  @IsOptional()
  @Matches(/^\d+(\.\d{1,2})?$/, { message: 'Costo invalido (ej: 150.00)' })
  @IsString()
  costo?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  marca?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  codigo_barras?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  unidad?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  permite_fracciones?: boolean;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  unidad_compra?: string;

  @ApiProperty({ required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  contenido_cantidad?: number;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  contenido_unidad?: string;

  @ApiProperty({ required: false, enum: ['fijo', 'escala'] })
  @IsOptional()
  @IsIn(['fijo', 'escala'])
  tipo_precio?: 'fijo' | 'escala';

  @ApiProperty({ required: false, enum: ['fijo', 'escala'] })
  @IsOptional()
  @IsIn(['fijo', 'escala'])
  tipo_costo?: 'fijo' | 'escala';

  @ApiProperty({ required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_minimo?: number;

  @ApiProperty({ required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  stock_maximo?: number | null;

  @ApiProperty({ required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  garantia_meses?: number | null;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  es_materia_prima?: boolean;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  ubicacion?: string;

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado?: 'activo' | 'inactivo';

  @ApiProperty({ required: false, enum: ['activo', 'inactivo'] })
  @IsOptional()
  @IsIn(['activo', 'inactivo'])
  estado_operativo?: 'activo' | 'inactivo';

  @ApiProperty({ required: false })
  @IsOptional()
  @IsObject()
  atributos?: Record<string, unknown>;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  motivo?: string;
}

/**
 * Superficie de edición del catálogo unificado. Sustituye las rutas legacy
 * `/catalog/productos*` y `/catalog/servicios*`: el tipo se infiere del item.
 */
@ApiTags('Catálogo · Items')
@ApiBearerAuth()
@Controller('api/v2/catalog/items')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.CATALOGO_ITEMS)
export class ItemsCatalogoController {
  constructor(
    private readonly items: ItemsService,
    private readonly productos: ProductosService,
    private readonly servicios: ServiciosService,
  ) {}

  @Patch(':id')
  @Permisos('catalogo.escribir')
  @ApiOperation({ summary: 'Editar un item (producto, servicio o alquiler)' })
  @ApiParam({ name: 'id', description: 'Id del item' })
  @ApiOkResponse({ type: ItemResponseDto, description: 'Item actualizado' })
  async actualizar(
    @Param('id') id: string,
    @Body() dto: ActualizarItemCatalogoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto | Servicio | void> {
    const item = await this.itemConEmpresa(id, usuario);
    if (item.tipo === 'producto') {
      return this.productos.actualizar(
        id,
        {
          nombre: dto.nombre,
          descripcion: dto.descripcion,
          categoria: dto.categoria,
          precio_base: dto.precio_base ?? dto.precio,
          stock: dto.stock,
          costo: dto.costo,
          marca: dto.marca,
          codigo_barras: dto.codigo_barras,
          unidad: dto.unidad,
          permite_fracciones: dto.permite_fracciones,
          unidad_compra: dto.unidad_compra,
          contenido_cantidad: dto.contenido_cantidad,
          contenido_unidad: dto.contenido_unidad,
          tipo_precio: dto.tipo_precio,
          tipo_costo: dto.tipo_costo,
          stock_minimo: dto.stock_minimo,
          stock_maximo: dto.stock_maximo,
          garantia_meses: dto.garantia_meses,
          es_materia_prima: dto.es_materia_prima,
          ubicacion: dto.ubicacion,
          estado_operativo: dto.estado_operativo ?? dto.estado,
          atributos: dto.atributos,
          motivo: dto.motivo,
        },
        item.empresa_id,
      );
    }
    if (item.tipo === 'servicio') {
      return this.servicios.actualizar(
        id,
        {
          nombre: dto.nombre,
          descripcion: dto.descripcion,
          categoria: dto.categoria,
          precio: dto.precio ?? dto.precio_base,
          costo: dto.costo,
          marca: dto.marca,
          codigo_barras: dto.codigo_barras,
          unidad: dto.unidad,
          estado: dto.estado ?? dto.estado_operativo,
          atributos: dto.atributos,
        },
        item.empresa_id,
      );
    }
    if (dto.estado === 'inactivo' || dto.estado_operativo === 'inactivo') {
      await this.items.eliminar(id, item.empresa_id);
      return;
    }
    throw new DomainError('ITEM_NO_EDITABLE', 'Solo productos y servicios admiten edición.');
  }

  @Delete(':id')
  @Permisos('catalogo.escribir')
  @HttpCode(204)
  @ApiOperation({ summary: 'Dar de baja un item (baja lógica)' })
  @ApiParam({ name: 'id', description: 'Id del item' })
  @ApiNoContentResponse({ description: 'Item dado de baja' })
  async eliminar(
    @Param('id') id: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    const item = await this.itemConEmpresa(id, usuario);
    if (item.tipo === 'producto') {
      await this.productos.eliminar(id, item.empresa_id);
      return;
    }
    if (item.tipo === 'servicio') {
      await this.servicios.eliminar(id, item.empresa_id);
      return;
    }
    await this.items.eliminar(id, item.empresa_id);
  }

  @Post(':id/variantes')
  @Permisos('catalogo.escribir')
  @ApiOperation({ summary: 'Agregar una variante a un producto' })
  @ApiParam({ name: 'id', description: 'Id del producto padre' })
  @ApiCreatedResponse({ type: ItemResponseDto, description: 'Variante creada' })
  async crearVariante(
    @Param('id') id: string,
    @Body() dto: VarianteItemRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const item = await this.itemConEmpresa(id, usuario);
    return this.productos.crearVariante(id, dto, item.empresa_id);
  }

  @Patch(':id/variantes/:varianteId')
  @Permisos('catalogo.escribir')
  @ApiOperation({ summary: 'Editar una variante (precio/stock/atributos)' })
  @ApiParam({ name: 'id', description: 'Id del producto padre' })
  @ApiParam({ name: 'varianteId', description: 'Id de la variante' })
  @ApiOkResponse({ type: ItemResponseDto, description: 'Variante actualizada' })
  async actualizarVariante(
    @Param('id') id: string,
    @Param('varianteId') varianteId: string,
    @Body() dto: ActualizarItemCatalogoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const item = await this.itemConEmpresa(id, usuario);
    return this.productos.actualizarVariante(
      id,
      varianteId,
      {
        nombre: dto.nombre,
        descripcion: dto.descripcion,
        categoria: dto.categoria,
        precio_base: dto.precio_base ?? dto.precio,
        stock: dto.stock,
        atributos: dto.atributos,
        motivo: dto.motivo,
      },
      item.empresa_id,
    );
  }

  @Delete(':id/variantes/:varianteId')
  @Permisos('catalogo.escribir')
  @HttpCode(204)
  @ApiOperation({ summary: 'Eliminar una variante' })
  @ApiParam({ name: 'id', description: 'Id del producto padre' })
  @ApiParam({ name: 'varianteId', description: 'Id de la variante' })
  @ApiNoContentResponse({ description: 'Variante eliminada' })
  async eliminarVariante(
    @Param('id') id: string,
    @Param('varianteId') varianteId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    const item = await this.itemConEmpresa(id, usuario);
    await this.productos.eliminarVariante(id, varianteId, item.empresa_id);
  }

  @Post(':id/imagenes')
  @Permisos('catalogo.escribir')
  @ApiOperation({ summary: 'Agregar una imagen a un item (máx. 3)' })
  @ApiParam({ name: 'id', description: 'Id del item' })
  @ApiCreatedResponse({ type: ItemResponseDto, description: 'Imagen agregada' })
  async agregarImagen(
    @Param('id') id: string,
    @Body() dto: ImagenItemRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Producto> {
    const item = await this.itemConEmpresa(id, usuario);
    return this.productos.agregarImagen(id, dto, item.empresa_id);
  }

  @Delete(':id/imagenes/:imagenId')
  @Permisos('catalogo.escribir')
  @HttpCode(204)
  @ApiOperation({ summary: 'Eliminar una imagen de un item' })
  @ApiParam({ name: 'id', description: 'Id del item' })
  @ApiParam({ name: 'imagenId', description: 'Id de la relación item-imagen' })
  @ApiNoContentResponse({ description: 'Imagen eliminada' })
  async eliminarImagen(
    @Param('id') id: string,
    @Param('imagenId') imagenId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    const item = await this.itemConEmpresa(id, usuario);
    await this.productos.eliminarImagen(id, imagenId, item.empresa_id);
  }

  /** Empresario: su tenant. Admin: la empresa del item. */
  private async itemConEmpresa(id: string, usuario: UsuarioContexto) {
    if (usuario.rol !== ROLES.SUPER_ADMIN && usuario.rol !== ROLES.ADMIN) {
      if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
      const item = await this.items.encontrarPorId(id, usuario.tenant_id);
      if (!item) throw new NotFoundError('Item', id);
      return item;
    }
    const item = await this.items.encontrarPorId(id);
    if (!item) throw new NotFoundError('Item', id);
    return item;
  }
}
