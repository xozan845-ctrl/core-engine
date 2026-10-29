import { ApiProperty, PartialType } from '@nestjs/swagger';
import { Type } from 'class-transformer';
import {
  ArrayMinSize,
  IsArray,
  IsBoolean,
  IsDateString,
  IsEmail,
  IsIn,
  IsInt,
  IsNumber,
  IsOptional,
  IsString,
  Max,
  Min,
  MinLength,
  ValidateNested,
} from 'class-validator';

const TIPOS_CUENTA_BANCO = ['ahorro', 'corriente', 'wallet'] as const;
const SIMBOLOGIAS = ['EAN13', 'CODE128'] as const;
const METODOS_PAGO = ['efectivo', 'tarjeta', 'transferencia', 'credito', 'otro'] as const;
const TIPOS_GASTO = ['inmediato', 'programado', 'recurrente'] as const;
const ESTADOS_COMPRA = ['recibida', 'pendiente', 'anulada'] as const;
const ORIGENES_COMPRA = ['productos', 'directa'] as const;
const TIPOS_COSTO = ['inventario', 'servicio', 'gasto', 'otro'] as const;

/** Línea de producto o servicio de una compra u orden de compra. */
export class LineaCompraRequestDto {
  @ApiProperty({ description: 'Id del producto del catálogo', required: false, nullable: true })
  @IsOptional()
  @IsString()
  productId?: string | null;

  @ApiProperty({ description: 'Nombre visible de la línea' })
  @IsString()
  @MinLength(1)
  name: string;

  @ApiProperty({ description: 'SKU del producto', required: false })
  @IsOptional()
  @IsString()
  sku?: string;

  @ApiProperty({ description: 'Cantidad comprada', minimum: 1 })
  @IsNumber()
  @Min(1)
  quantity: number;

  @ApiProperty({ description: 'Costo unitario en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  unitCostCents: number;

  @ApiProperty({ description: 'Precio de venta en centavos', required: false, nullable: true, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  salePriceCents?: number | null;
}

export class CrearCuentaBancoRequestDto {
  @ApiProperty({ description: 'Banco', example: 'BAC' })
  @IsString()
  @MinLength(1)
  bank: string;

  @ApiProperty({ description: 'Alias visible de la cuenta', example: 'Operativa' })
  @IsString()
  @MinLength(1)
  alias: string;

  @ApiProperty({ description: 'Número de cuenta' })
  @IsString()
  @MinLength(1)
  accountNumber: string;

  @ApiProperty({ description: 'Tipo de cuenta', enum: TIPOS_CUENTA_BANCO })
  @IsIn(TIPOS_CUENTA_BANCO)
  type: (typeof TIPOS_CUENTA_BANCO)[number];

  @ApiProperty({ description: 'Saldo inicial en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  balanceCents: number;
}

export class ActualizarCuentaBancoRequestDto extends PartialType(CrearCuentaBancoRequestDto) {}

export class CrearCreditoRequestDto {
  @ApiProperty({ description: 'Cliente que recibe el crédito' })
  @IsString()
  @MinLength(1)
  customer: string;

  @ApiProperty({ description: 'Concepto del crédito' })
  @IsString()
  @MinLength(3)
  concept: string;

  @ApiProperty({ description: 'Fecha de emisión (yyyy-mm-dd)', required: false })
  @IsOptional()
  @IsDateString()
  date?: string;

  @ApiProperty({ description: 'Fecha de vencimiento (yyyy-mm-dd)' })
  @IsDateString()
  dueDate: string;

  @ApiProperty({ description: 'Importe en centavos', minimum: 1 })
  @IsInt()
  @Min(1)
  amountCents: number;
}

export class CrearGastoRequestDto {
  @ApiProperty({ description: 'Fecha del gasto (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Concepto del gasto' })
  @IsString()
  @MinLength(3)
  concept: string;

  @ApiProperty({ description: 'Categoría visible (puede ser personalizada)' })
  @IsString()
  @MinLength(1)
  category: string;

  @ApiProperty({ description: 'Tipo de gasto', enum: TIPOS_GASTO })
  @IsIn(TIPOS_GASTO)
  type: (typeof TIPOS_GASTO)[number];

  @ApiProperty({ description: 'Importe en centavos', minimum: 1 })
  @IsInt()
  @Min(1)
  amountCents: number;

  @ApiProperty({ description: 'Método de pago', enum: METODOS_PAGO })
  @IsIn(METODOS_PAGO)
  paymentMethod: (typeof METODOS_PAGO)[number];

  @ApiProperty({ description: 'Incluye IVA', required: false, default: false })
  @IsOptional()
  @IsBoolean()
  includesVat?: boolean;

  @ApiProperty({ description: 'Retención de IR en centavos', required: false, minimum: 0, default: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  irRetentionCents?: number;

  @ApiProperty({ description: 'Proveedor, empleado o beneficiario', required: false })
  @IsOptional()
  @IsString()
  beneficiary?: string;

  @ApiProperty({ description: 'Notas', required: false })
  @IsOptional()
  @IsString()
  notes?: string;

  @ApiProperty({ description: 'Fecha programada (yyyy-mm-dd)', required: false })
  @IsOptional()
  @IsDateString()
  scheduledFor?: string;

  @ApiProperty({ description: 'Etiqueta de recurrencia', required: false })
  @IsOptional()
  @IsString()
  recurrence?: string;
}

export class ActualizarGastoRequestDto extends PartialType(CrearGastoRequestDto) {}

export class CrearCuentaPorPagarRequestDto {
  @ApiProperty({ description: 'Proveedor' })
  @IsString()
  @MinLength(1)
  supplier: string;

  @ApiProperty({ description: 'Concepto de la obligación' })
  @IsString()
  @MinLength(3)
  concept: string;

  @ApiProperty({ description: 'Fecha de vencimiento (yyyy-mm-dd)' })
  @IsDateString()
  dueDate: string;

  @ApiProperty({ description: 'Importe en centavos', minimum: 1 })
  @IsInt()
  @Min(1)
  amountCents: number;
}

export class CrearProveedorRequestDto {
  @ApiProperty({ description: 'Nombre del proveedor' })
  @IsString()
  @MinLength(1)
  name: string;

  @ApiProperty({ description: 'RUC', required: false })
  @IsOptional()
  @IsString()
  ruc?: string;

  @ApiProperty({ description: 'Teléfono', required: false })
  @IsOptional()
  @IsString()
  phone?: string;

  @ApiProperty({ description: 'Correo', required: false })
  @IsOptional()
  @IsEmail()
  email?: string;

  @ApiProperty({ description: 'Dirección', required: false })
  @IsOptional()
  @IsString()
  address?: string;
}

export class CrearCompraRequestDto {
  @ApiProperty({ description: 'Número del documento; si se omite se genera', required: false })
  @IsOptional()
  @IsString()
  number?: string;

  @ApiProperty({ description: 'Fecha de la compra (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Id del proveedor', required: false, nullable: true })
  @IsOptional()
  @IsString()
  supplierId?: string | null;

  @ApiProperty({ description: 'Nombre del proveedor', required: false })
  @IsOptional()
  @IsString()
  supplierName?: string;

  @ApiProperty({ description: 'Método de pago', enum: METODOS_PAGO })
  @IsIn(METODOS_PAGO)
  paymentMethod: (typeof METODOS_PAGO)[number];

  @ApiProperty({ description: 'Estado del documento', enum: ESTADOS_COMPRA })
  @IsIn(ESTADOS_COMPRA)
  status: (typeof ESTADOS_COMPRA)[number];

  @ApiProperty({ description: 'Origen de la compra', enum: ORIGENES_COMPRA })
  @IsIn(ORIGENES_COMPRA)
  origin: (typeof ORIGENES_COMPRA)[number];

  @ApiProperty({ description: 'Tipo de costo', enum: TIPOS_COSTO })
  @IsIn(TIPOS_COSTO)
  costType: (typeof TIPOS_COSTO)[number];

  @ApiProperty({ description: 'Líneas de la compra', type: [LineaCompraRequestDto] })
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => LineaCompraRequestDto)
  lines: LineaCompraRequestDto[];

  @ApiProperty({ description: 'Notas', required: false })
  @IsOptional()
  @IsString()
  notes?: string;

  @ApiProperty({ description: 'Retención informativa en centavos', required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  retentionCents?: number;

  @ApiProperty({ description: 'Usuario que registra la compra', required: false })
  @IsOptional()
  @IsString()
  userName?: string;
}

export class CrearCompraDirectaRequestDto {
  @ApiProperty({ description: 'Número del documento; si se omite se genera', required: false })
  @IsOptional()
  @IsString()
  number?: string;

  @ApiProperty({ description: 'Fecha de la compra (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Id del proveedor', required: false, nullable: true })
  @IsOptional()
  @IsString()
  supplierId?: string | null;

  @ApiProperty({ description: 'Nombre del proveedor', required: false })
  @IsOptional()
  @IsString()
  supplierName?: string;

  @ApiProperty({ description: 'Concepto del gasto directo' })
  @IsString()
  @MinLength(1)
  concept: string;

  @ApiProperty({ description: 'Tipo de costo', enum: TIPOS_COSTO })
  @IsIn(TIPOS_COSTO)
  costType: (typeof TIPOS_COSTO)[number];

  @ApiProperty({ description: 'Importe en centavos', minimum: 1 })
  @IsInt()
  @Min(1)
  amountCents: number;

  @ApiProperty({ description: 'Método de pago', enum: METODOS_PAGO })
  @IsIn(METODOS_PAGO)
  paymentMethod: (typeof METODOS_PAGO)[number];

  @ApiProperty({ description: 'Notas', required: false })
  @IsOptional()
  @IsString()
  notes?: string;

  @ApiProperty({ description: 'Retención informativa en centavos', required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  retentionCents?: number;
}

export class CrearCompraProductoNuevoRequestDto {
  @ApiProperty({ description: 'Número del documento; si se omite se genera', required: false })
  @IsOptional()
  @IsString()
  number?: string;

  @ApiProperty({ description: 'Fecha de la compra (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Id del proveedor', required: false, nullable: true })
  @IsOptional()
  @IsString()
  supplierId?: string | null;

  @ApiProperty({ description: 'Nombre del proveedor', required: false })
  @IsOptional()
  @IsString()
  supplierName?: string;

  @ApiProperty({ description: 'Nombre del producto nuevo' })
  @IsString()
  @MinLength(1)
  productName: string;

  @ApiProperty({ description: 'Categoría', required: false })
  @IsOptional()
  @IsString()
  category?: string;

  @ApiProperty({ description: 'SKU', required: false })
  @IsOptional()
  @IsString()
  sku?: string;

  @ApiProperty({ description: 'Método de pago', enum: METODOS_PAGO })
  @IsIn(METODOS_PAGO)
  paymentMethod: (typeof METODOS_PAGO)[number];

  @ApiProperty({ description: 'Costo unitario en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  unitCostCents: number;

  @ApiProperty({ description: 'Precio de venta en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  salePriceCents: number;

  @ApiProperty({ description: 'Cantidad', minimum: 1 })
  @IsNumber()
  @Min(1)
  quantity: number;

  @ApiProperty({ description: 'Notas', required: false })
  @IsOptional()
  @IsString()
  notes?: string;
}

export class CrearOrdenCompraRequestDto {
  @ApiProperty({ description: 'Número de la orden; si se omite se genera', required: false })
  @IsOptional()
  @IsString()
  number?: string;

  @ApiProperty({ description: 'Fecha de la orden (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Id del proveedor', required: false, nullable: true })
  @IsOptional()
  @IsString()
  supplierId?: string | null;

  @ApiProperty({ description: 'Nombre del proveedor', required: false })
  @IsOptional()
  @IsString()
  supplierName?: string;

  @ApiProperty({ description: 'Líneas de la orden', type: [LineaCompraRequestDto] })
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => LineaCompraRequestDto)
  lines: LineaCompraRequestDto[];

  @ApiProperty({ description: 'Notas', required: false })
  @IsOptional()
  @IsString()
  notes?: string;
}

export class CrearMetodoPagoRequestDto {
  @ApiProperty({ description: 'Nombre visible del método' })
  @IsString()
  @MinLength(1)
  name: string;

  @ApiProperty({ description: 'Tipo funcional (efectivo, tarjeta, transferencia…)' })
  @IsString()
  @MinLength(1)
  kind: string;

  @ApiProperty({ description: 'Icono', required: false })
  @IsOptional()
  @IsString()
  icon?: string;

  @ApiProperty({ description: 'Activo', required: false, default: true })
  @IsOptional()
  @IsBoolean()
  active?: boolean;
}

export class ActualizarMetodoPagoRequestDto extends PartialType(CrearMetodoPagoRequestDto) {}

export class CrearMovimientoCajaRequestDto {
  @ApiProperty({ description: 'Fecha local (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Dirección del movimiento', enum: ['ingreso', 'egreso'] })
  @IsIn(['ingreso', 'egreso'])
  type: 'ingreso' | 'egreso';

  @ApiProperty({ description: 'Concepto' })
  @IsString()
  @MinLength(1)
  concept: string;

  @ApiProperty({ description: 'Importe en centavos', minimum: 1 })
  @IsInt()
  @Min(1)
  amountCents: number;

  @ApiProperty({ description: 'Método de pago (id o etiqueta)' })
  @IsString()
  @MinLength(1)
  paymentMethod: string;

  @ApiProperty({ description: 'Categoría funcional', required: false })
  @IsOptional()
  @IsString()
  category?: string;

  @ApiProperty({ description: 'Moneda', required: false, enum: ['NIO', 'USD'] })
  @IsOptional()
  @IsIn(['NIO', 'USD'])
  currency?: 'NIO' | 'USD';

  @ApiProperty({ description: 'Importe original capturado en centavos', required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  originalAmountCents?: number;

  @ApiProperty({ description: 'Tipo de cambio aplicado', required: false, minimum: 0 })
  @IsOptional()
  @IsNumber()
  @Min(0)
  exchangeRate?: number;

  @ApiProperty({ description: 'Referencia visible del documento', required: false })
  @IsOptional()
  @IsString()
  reference?: string;

  @ApiProperty({ description: 'Observaciones', required: false })
  @IsOptional()
  @IsString()
  observations?: string;

  @ApiProperty({ description: 'Origen del movimiento', required: false })
  @IsOptional()
  @IsString()
  source?: string;
}

export class CrearCierreCajaRequestDto {
  @ApiProperty({ description: 'Fecha del corte (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Saldo contado en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  countedBalanceCents: number;

  @ApiProperty({ description: 'Notas del cierre', required: false })
  @IsOptional()
  @IsString()
  notes?: string;
}

export class CrearFondoCajaChicaRequestDto {
  @ApiProperty({ description: 'Monto del fondo en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  amountCents: number;

  @ApiProperty({ description: 'Responsable del fondo' })
  @IsString()
  @MinLength(1)
  responsible: string;
}

export class CrearMovimientoCajaChicaRequestDto {
  @ApiProperty({ description: 'Fecha local (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Tipo de movimiento', enum: ['reposicion', 'gasto'] })
  @IsIn(['reposicion', 'gasto'])
  type: 'reposicion' | 'gasto';

  @ApiProperty({ description: 'Concepto' })
  @IsString()
  @MinLength(1)
  concept: string;

  @ApiProperty({ description: 'Importe en centavos', minimum: 1 })
  @IsInt()
  @Min(1)
  amountCents: number;
}

export class CrearTransferenciaCajaRequestDto {
  @ApiProperty({ description: 'Fecha local (yyyy-mm-dd)' })
  @IsDateString()
  date: string;

  @ApiProperty({ description: 'Id de la cuenta origen' })
  @IsString()
  @MinLength(1)
  fromId: string;

  @ApiProperty({ description: 'Etiqueta de la cuenta origen' })
  @IsString()
  @MinLength(1)
  fromLabel: string;

  @ApiProperty({ description: 'Tipo de cuenta origen', enum: ['cash', 'bank'] })
  @IsIn(['cash', 'bank'])
  fromKind: 'cash' | 'bank';

  @ApiProperty({ description: 'Id de la cuenta destino' })
  @IsString()
  @MinLength(1)
  toId: string;

  @ApiProperty({ description: 'Etiqueta de la cuenta destino' })
  @IsString()
  @MinLength(1)
  toLabel: string;

  @ApiProperty({ description: 'Tipo de cuenta destino', enum: ['cash', 'bank'] })
  @IsIn(['cash', 'bank'])
  toKind: 'cash' | 'bank';

  @ApiProperty({ description: 'Importe en centavos', minimum: 1 })
  @IsInt()
  @Min(1)
  amountCents: number;

  @ApiProperty({ description: 'Concepto de la transferencia' })
  @IsString()
  @MinLength(1)
  concept: string;

  @ApiProperty({ description: 'Notas', required: false })
  @IsOptional()
  @IsString()
  notes?: string;
}

export class ActualizarConfiguracionDocumentosRequestDto {
  @ApiProperty({ description: 'Tamaño del logo', enum: ['pequeno', 'mediano', 'grande'] })
  @IsIn(['pequeno', 'mediano', 'grande'])
  logoSize: 'pequeno' | 'mediano' | 'grande';

  @ApiProperty({ description: 'Color de cabecera', example: '#0f172a' })
  @IsString()
  headerColor: string;

  @ApiProperty({ description: 'Igualar color de tabla al de cabecera' })
  @IsBoolean()
  matchTableColor: boolean;

  @ApiProperty({ description: 'Mensaje del pie de página' })
  @IsString()
  footerMessage: string;

  @ApiProperty({ description: 'Mostrar RUC' })
  @IsBoolean()
  showRuc: boolean;

  @ApiProperty({ description: 'Mostrar dirección' })
  @IsBoolean()
  showAddress: boolean;

  @ApiProperty({ description: 'Mostrar teléfono' })
  @IsBoolean()
  showPhone: boolean;
}

export class CrearClienteComercioRequestDto {
  @ApiProperty({ description: 'Nombre del cliente' })
  @IsString()
  @MinLength(1)
  name: string;

  @ApiProperty({ description: 'Teléfono', required: false })
  @IsOptional()
  @IsString()
  phone?: string;

  @ApiProperty({ description: 'Correo', required: false })
  @IsOptional()
  @IsEmail()
  email?: string;

  @ApiProperty({ description: 'Empresa del cliente', required: false })
  @IsOptional()
  @IsString()
  company?: string;

  @ApiProperty({ description: 'Id del comprador en Core Engine', required: false })
  @IsOptional()
  @IsString()
  buyerId?: string;

  @ApiProperty({ description: 'Origen', required: false, enum: ['manual', 'import'] })
  @IsOptional()
  @IsIn(['manual', 'import'])
  source?: 'manual' | 'import';
}

export class ActualizarClienteComercioRequestDto extends PartialType(CrearClienteComercioRequestDto) {}

export class ImportarClientesRequestDto {
  @ApiProperty({ description: 'Filas a importar (máximo 500)', type: [CrearClienteComercioRequestDto] })
  @IsArray()
  @ArrayMinSize(1)
  @ValidateNested({ each: true })
  @Type(() => CrearClienteComercioRequestDto)
  clientes: CrearClienteComercioRequestDto[];
}

export class CrearCodigoBarrasRequestDto {
  @ApiProperty({ description: 'Producto al que pertenece la etiqueta' })
  @IsString()
  @MinLength(1)
  productName: string;

  @ApiProperty({ description: 'SKU del producto' })
  @IsString()
  @MinLength(1)
  sku: string;

  @ApiProperty({ description: 'Código; vacío = se genera según la simbología', required: false })
  @IsOptional()
  @IsString()
  code?: string;

  @ApiProperty({ description: 'Simbología', enum: SIMBOLOGIAS })
  @IsIn(SIMBOLOGIAS)
  symbology: (typeof SIMBOLOGIAS)[number];

  @ApiProperty({ description: 'Copias a imprimir (1-100)', minimum: 1, maximum: 100 })
  @IsInt()
  @Min(1)
  @Max(100)
  copies: number;
}
