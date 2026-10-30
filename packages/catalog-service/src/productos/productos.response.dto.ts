import { ApiProperty } from '@nestjs/swagger';

export class ImagenProductoResponseDto {
  @ApiProperty({ description: 'Id de la relación producto-imagen', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id del objeto en storage-service', format: 'uuid' })
  objeto_id: string;

  @ApiProperty({
    description: 'Ruta pública relativa servida por storage-service',
    example: '/api/v1/storage/public/objetos/8f0c...',
  })
  url: string;

  @ApiProperty({ description: 'Orden de la imagen (0-2)', minimum: 0, maximum: 2 })
  orden: number;

  @ApiProperty({ description: 'Imagen principal del producto/variante' })
  es_principal: boolean;
}

export class VarianteProductoResponseDto {
  @ApiProperty({ description: 'Id de la variante', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'Id del producto padre', format: 'uuid' })
  producto_padre_id: string;

  @ApiProperty({ description: 'SKU de la variante', example: 'SKU-001-ROJO-M' })
  sku: string;

  @ApiProperty({ description: 'Nombre visible de la variante' })
  nombre: string;

  @ApiProperty({
    description: 'Atributos de la variante (color, talla, etc.)',
    type: 'object',
    additionalProperties: true,
  })
  atributos: Record<string, unknown>;

  @ApiProperty({ description: 'Precio base en córdobas con dos decimales', example: '1500.00' })
  precio_base: string;

  @ApiProperty({ description: 'Stock disponible', example: 10 })
  stock: number;

  @ApiProperty({ description: 'Estado de la variante', enum: ['disponible', 'agotado'] })
  estado: 'disponible' | 'agotado';

  @ApiProperty({ description: 'Imágenes de la variante (máx. 3)', type: () => [ImagenProductoResponseDto] })
  imagenes: ImagenProductoResponseDto[];

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;
}

export class ProductoResponseDto {
  @ApiProperty({ description: 'Empresa propietaria', format: 'uuid' })
  empresa_id: string;
  @ApiProperty({ description: 'Id del producto', format: 'uuid' })
  id: string;

  @ApiProperty({ description: 'SKU del producto (único por empresa)', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Nombre del producto' })
  nombre: string;

  @ApiProperty({ description: 'Descripción del producto' })
  descripcion: string;

  @ApiProperty({ description: 'Categoría del producto' })
  categoria: string;

  @ApiProperty({
    description: 'Precio base en córdobas con dos decimales',
    example: '1000.00',
  })
  precio_base: string;

  @ApiProperty({ description: 'Stock disponible (neto de reservas)', example: 10 })
  stock: number;

  @ApiProperty({ description: 'Estado del producto', enum: ['disponible', 'agotado'], example: 'disponible' })
  estado: 'disponible' | 'agotado';

  @ApiProperty({ description: 'Estado comercial del producto', enum: ['activo', 'inactivo'], required: false })
  estado_operativo: 'activo' | 'inactivo';

  @ApiProperty({ description: 'Costo en córdobas con dos decimales', example: '150.00', required: false })
  costo: string;

  @ApiProperty({ description: 'Marca', required: false })
  marca: string;

  @ApiProperty({ description: 'Código de barras único por empresa', required: false })
  codigo_barras: string;

  @ApiProperty({ description: 'Unidad de venta', example: 'Unidad (u)', required: false })
  unidad: string;

  @ApiProperty({ description: 'Permite fracciones', required: false })
  permite_fracciones: boolean;

  @ApiProperty({ description: 'Unidad de compra', required: false })
  unidad_compra: string;

  @ApiProperty({ description: 'Contenido por unidad', required: false })
  contenido_cantidad: number;

  @ApiProperty({ description: 'Unidad del contenido', required: false })
  contenido_unidad: string;

  @ApiProperty({ description: 'Tipo de precio', enum: ['fijo', 'escala'], required: false })
  tipo_precio: 'fijo' | 'escala';

  @ApiProperty({ description: 'Tipo de costo', enum: ['fijo', 'escala'], required: false })
  tipo_costo: 'fijo' | 'escala';

  @ApiProperty({ description: 'Stock mínimo para alertas', required: false })
  stock_minimo: number;

  @ApiProperty({ description: 'Stock máximo', nullable: true, required: false })
  stock_maximo: number | null;

  @ApiProperty({ description: 'Garantía en meses', nullable: true, required: false })
  garantia_meses: number | null;

  @ApiProperty({ description: 'Es materia prima', required: false })
  es_materia_prima: boolean;

  @ApiProperty({ description: 'Ubicación física', required: false })
  ubicacion: string;

  @ApiProperty({ description: 'Última actualización (ISO 8601 UTC)', format: 'date-time', required: false })
  actualizado_en: string;

  @ApiProperty({ description: 'Fecha de creación (ISO 8601 UTC)', format: 'date-time' })
  creado_en: string;

  @ApiProperty({ description: 'Producto padre si esta fila es una variante', required: false, nullable: true, format: 'uuid' })
  producto_padre_id: string | null;

  @ApiProperty({
    description: 'Atributos de la variante (color, talla, etc.)',
    type: 'object',
    additionalProperties: true,
    selfRequired: false,
  })
  atributos: Record<string, unknown>;

  @ApiProperty({ description: 'El producto tiene variantes vendibles', required: false })
  tiene_variantes: boolean;

  @ApiProperty({
    description: 'Imágenes del producto (máx. 3)',
    type: () => [ImagenProductoResponseDto],
    required: false,
  })
  imagenes: ImagenProductoResponseDto[];

  @ApiProperty({
    description: 'Variantes (solo en el detalle del padre)',
    type: () => [VarianteProductoResponseDto],
    required: false,
  })
  variantes?: VarianteProductoResponseDto[];

  @ApiProperty({ description: 'Precio mínimo mostrable ("desde")', required: false, example: '1000.00' })
  precio_desde: string;

  @ApiProperty({ description: 'Stock agregado de las variantes o el propio', required: false, example: 25 })
  stock_total: number;
}

export class PaginaProductosResponseDto {
  @ApiProperty({ description: 'Productos de la página', type: () => [ProductoResponseDto] })
  items: ProductoResponseDto[];

  @ApiProperty({ description: 'Total de productos que cumplen el filtro', example: 42 })
  total: number;

  @ApiProperty({ description: 'Página actual (1-based)', example: 1 })
  pagina: number;

  @ApiProperty({ description: 'Tamaño de página', example: 20 })
  limite: number;

  @ApiProperty({ description: 'Total de páginas', example: 3 })
  paginas: number;
}

export class LoteProductoResponseDto {
  @ApiProperty({ description: 'Empresa dueña del producto', format: 'uuid', required: false })
  empresa_id: string;

  @ApiProperty({ description: 'SKU del producto (único por empresa)', example: 'SKU-001' })
  sku: string;

  @ApiProperty({ description: 'Stock disponible', example: 10 })
  stock: number;

  @ApiProperty({
    description: 'Estado del producto',
    enum: ['disponible', 'agotado'],
    example: 'disponible',
  })
  estado: string;
}
