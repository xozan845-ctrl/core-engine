import { ApiProperty } from '@nestjs/swagger';

/** Campos de control comunes a todos los registros del libro de comercio. */
export class RegistroComercioResponseDto {
  @ApiProperty({ format: 'uuid' })
  id: string;

  @ApiProperty({ format: 'date-time' })
  createdAt: string;

  @ApiProperty({ format: 'date-time' })
  updatedAt: string;
}

export class CuentaBancoResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  bank: string;

  @ApiProperty()
  alias: string;

  @ApiProperty()
  accountNumber: string;

  @ApiProperty({ enum: ['ahorro', 'corriente', 'wallet'] })
  type: string;

  @ApiProperty({ description: 'Saldo en centavos' })
  balanceCents: number;
}

export class CreditoResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  customer: string;

  @ApiProperty()
  concept: string;

  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty({ format: 'date' })
  dueDate: string;

  @ApiProperty({ description: 'Importe original en centavos' })
  amountCents: number;

  @ApiProperty({ description: 'Saldo pendiente en centavos' })
  balanceCents: number;
}

export class GastoResponseDto extends RegistroComercioResponseDto {
  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty()
  concept: string;

  @ApiProperty()
  category: string;

  @ApiProperty({ enum: ['inmediato', 'programado', 'recurrente'] })
  type: string;

  @ApiProperty({ description: 'Importe en centavos' })
  amountCents: number;

  @ApiProperty()
  paymentMethod: string;

  @ApiProperty({ enum: ['pendiente', 'pagado', 'cancelado'] })
  status: string;

  @ApiProperty()
  includesVat: boolean;

  @ApiProperty({ description: 'Retención de IR en centavos' })
  irRetentionCents: number;

  @ApiProperty()
  beneficiary: string;

  @ApiProperty()
  notes: string;

  @ApiProperty({ format: 'date' })
  scheduledFor: string;

  @ApiProperty()
  recurrence: string;
}

export class CuentaPorPagarResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  supplier: string;

  @ApiProperty()
  concept: string;

  @ApiProperty({ format: 'date' })
  dueDate: string;

  @ApiProperty({ description: 'Importe en centavos' })
  amountCents: number;

  @ApiProperty({ enum: ['pendiente', 'pagada'] })
  status: string;

  @ApiProperty({ format: 'date' })
  paidAt: string;
}

export class ProveedorResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  name: string;

  @ApiProperty()
  ruc: string;

  @ApiProperty()
  phone: string;

  @ApiProperty()
  email: string;

  @ApiProperty()
  address: string;
}

export class LineaCompraResponseDto {
  @ApiProperty()
  id: string;

  @ApiProperty({ nullable: true })
  productId: string | null;

  @ApiProperty()
  name: string;

  @ApiProperty()
  sku: string;

  @ApiProperty()
  quantity: number;

  @ApiProperty({ description: 'Costo unitario en centavos' })
  unitCostCents: number;

  @ApiProperty({ description: 'Precio de venta en centavos', nullable: true })
  salePriceCents: number | null;
}

export class CompraResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  number: string;

  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty({ nullable: true })
  supplierId: string | null;

  @ApiProperty()
  supplierName: string;

  @ApiProperty()
  paymentMethod: string;

  @ApiProperty({ enum: ['recibida', 'pendiente', 'anulada'] })
  status: string;

  @ApiProperty({ enum: ['productos', 'directa'] })
  origin: string;

  @ApiProperty()
  costType: string;

  @ApiProperty({ type: [LineaCompraResponseDto] })
  lines: LineaCompraResponseDto[];

  @ApiProperty({ description: 'Subtotal en centavos' })
  subtotalCents: number;

  @ApiProperty({ description: 'Retención en centavos' })
  retentionCents: number;

  @ApiProperty({ description: 'Total en centavos' })
  totalCents: number;

  @ApiProperty()
  notes: string;

  @ApiProperty()
  userName: string;
}

export class OrdenCompraResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  number: string;

  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty({ nullable: true })
  supplierId: string | null;

  @ApiProperty()
  supplierName: string;

  @ApiProperty({ type: [LineaCompraResponseDto] })
  lines: LineaCompraResponseDto[];

  @ApiProperty({ description: 'Total en centavos' })
  totalCents: number;

  @ApiProperty({ enum: ['pendiente', 'convertida', 'anulada'] })
  status: string;

  @ApiProperty()
  notes: string;
}

export class MetodoPagoResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  name: string;

  @ApiProperty()
  kind: string;

  @ApiProperty()
  icon: string;

  @ApiProperty()
  active: boolean;
}

export class MovimientoCajaResponseDto extends RegistroComercioResponseDto {
  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty({ enum: ['ingreso', 'egreso'] })
  type: string;

  @ApiProperty()
  concept: string;

  @ApiProperty({ description: 'Importe en centavos' })
  amountCents: number;

  @ApiProperty({ enum: ['NIO', 'USD'] })
  currency: string;

  @ApiProperty({ description: 'Importe original en centavos' })
  originalAmountCents: number;

  @ApiProperty()
  exchangeRate: number;

  @ApiProperty()
  paymentMethod: string;

  @ApiProperty()
  category: string;

  @ApiProperty()
  source: string;

  @ApiProperty({ enum: ['completado', 'anulado'] })
  status: string;

  @ApiProperty()
  reference: string;

  @ApiProperty()
  observations: string;

  @ApiProperty({ required: false })
  transferId?: string;
}

export class CierreCajaResponseDto extends RegistroComercioResponseDto {
  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty({ description: 'Saldo inicial en centavos' })
  openingBalanceCents: number;

  @ApiProperty({ description: 'Ingresos del día en centavos' })
  incomeCents: number;

  @ApiProperty({ description: 'Egresos del día en centavos' })
  expenseCents: number;

  @ApiProperty({ description: 'Saldo esperado en centavos' })
  expectedBalanceCents: number;

  @ApiProperty({ description: 'Saldo contado en centavos' })
  countedBalanceCents: number;

  @ApiProperty({ description: 'Diferencia en centavos' })
  differenceCents: number;

  @ApiProperty()
  movementsCount: number;

  @ApiProperty()
  notes: string;

  @ApiProperty({ format: 'date-time' })
  closedAt: string;
}

export class CierreCajaPreviewResponseDto {
  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty({ description: 'Saldo inicial en centavos' })
  openingBalanceCents: number;

  @ApiProperty({ description: 'Ingresos del día en centavos' })
  incomeCents: number;

  @ApiProperty({ description: 'Egresos del día en centavos' })
  expenseCents: number;

  @ApiProperty({ description: 'Saldo esperado en centavos' })
  expectedBalanceCents: number;

  @ApiProperty()
  movementsCount: number;

  @ApiProperty({ type: CierreCajaResponseDto, nullable: true })
  existing: CierreCajaResponseDto | null;
}

export class FondoCajaChicaResponseDto extends RegistroComercioResponseDto {
  @ApiProperty({ description: 'Monto del fondo en centavos' })
  amountCents: number;

  @ApiProperty()
  responsible: string;
}

export class MovimientoCajaChicaResponseDto extends RegistroComercioResponseDto {
  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty({ enum: ['reposicion', 'gasto'] })
  type: string;

  @ApiProperty()
  concept: string;

  @ApiProperty({ description: 'Importe en centavos' })
  amountCents: number;
}

export class TransferenciaCajaResponseDto extends RegistroComercioResponseDto {
  @ApiProperty({ format: 'date' })
  date: string;

  @ApiProperty()
  fromId: string;

  @ApiProperty()
  fromLabel: string;

  @ApiProperty({ enum: ['cash', 'bank'] })
  fromKind: string;

  @ApiProperty()
  toId: string;

  @ApiProperty()
  toLabel: string;

  @ApiProperty({ enum: ['cash', 'bank'] })
  toKind: string;

  @ApiProperty({ description: 'Importe en centavos' })
  amountCents: number;

  @ApiProperty()
  concept: string;

  @ApiProperty()
  notes: string;

  @ApiProperty({ enum: ['completado', 'anulado'] })
  status: string;
}

export class ConfiguracionDocumentosResponseDto extends RegistroComercioResponseDto {
  @ApiProperty({ enum: ['pequeno', 'mediano', 'grande'] })
  logoSize: string;

  @ApiProperty()
  headerColor: string;

  @ApiProperty()
  matchTableColor: boolean;

  @ApiProperty()
  footerMessage: string;

  @ApiProperty()
  showRuc: boolean;

  @ApiProperty()
  showAddress: boolean;

  @ApiProperty()
  showPhone: boolean;
}

export class ClienteComercioResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  name: string;

  @ApiProperty()
  phone: string;

  @ApiProperty()
  email: string;

  @ApiProperty()
  company: string;

  @ApiProperty({ required: false })
  buyerId?: string;

  @ApiProperty({ enum: ['order', 'manual', 'import'] })
  source: string;
}

export class ImportarClientesResponseDto {
  @ApiProperty()
  imported: number;

  @ApiProperty()
  skipped: number;

  @ApiProperty({ type: [String] })
  errors: string[];
}

export class CodigoBarrasResponseDto extends RegistroComercioResponseDto {
  @ApiProperty()
  productName: string;

  @ApiProperty()
  sku: string;

  @ApiProperty()
  code: string;

  @ApiProperty({ enum: ['EAN13', 'CODE128'] })
  symbology: string;

  @ApiProperty()
  copies: number;
}
