import {
  Body,
  Controller,
  Delete,
  Get,
  Param,
  Patch,
  Post,
  Put,
  Query,
} from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiQuery,
  ApiTags,
} from '@nestjs/swagger';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { ComercioService } from './comercio.service';
import {
  ActualizarClienteComercioRequestDto,
  ActualizarConfiguracionDocumentosRequestDto,
  ActualizarCuentaBancoRequestDto,
  ActualizarGastoRequestDto,
  ActualizarMetodoPagoRequestDto,
  CrearCierreCajaRequestDto,
  CrearClienteComercioRequestDto,
  CrearCodigoBarrasRequestDto,
  CrearCompraDirectaRequestDto,
  CrearCompraProductoNuevoRequestDto,
  CrearCompraRequestDto,
  CrearCreditoRequestDto,
  CrearCuentaBancoRequestDto,
  CrearCuentaPorPagarRequestDto,
  CrearFondoCajaChicaRequestDto,
  CrearGastoRequestDto,
  CrearMetodoPagoRequestDto,
  CrearMovimientoCajaChicaRequestDto,
  CrearMovimientoCajaRequestDto,
  CrearOrdenCompraRequestDto,
  CrearProveedorRequestDto,
  CrearTransferenciaCajaRequestDto,
  ImportarClientesRequestDto,
} from './comercio.dtos';
import {
  CierreCajaPreviewResponseDto,
  CierreCajaResponseDto,
  ClienteComercioResponseDto,
  CodigoBarrasResponseDto,
  CompraResponseDto,
  ConfiguracionDocumentosResponseDto,
  CreditoResponseDto,
  CuentaBancoResponseDto,
  CuentaPorPagarResponseDto,
  FondoCajaChicaResponseDto,
  GastoResponseDto,
  ImportarClientesResponseDto,
  MetodoPagoResponseDto,
  MovimientoCajaChicaResponseDto,
  MovimientoCajaResponseDto,
  OrdenCompraResponseDto,
  ProveedorResponseDto,
  TransferenciaCajaResponseDto,
} from './comercio.response.dto';

/**
 * Operación de comercio del workspace de empresa y vendedor: bancos, créditos,
 * gastos, cuentas por pagar, compras, caja, clientes y códigos de barras.
 * Sustituye la persistencia local del frontend (ADR-66 a ADR-72).
 */
@ApiTags('Comercio · Operación de tienda')
@ApiBearerAuth()
@Controller('api/v1/comercio')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ROLES.VENDEDOR)
export class ComercioController {
  constructor(private readonly comercio: ComercioService) {}

  // ── bancos ───────────────────────────────────────────────────────────────
  @Get('bancos-cuentas')
  @ApiOperation({ summary: 'Listar cuentas bancarias del workspace' })
  @ApiOkResponse({ type: [CuentaBancoResponseDto], description: 'Cuentas bancarias' })
  listarBancos(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarBancos(usuario);
  }

  @Post('bancos-cuentas')
  @ApiOperation({ summary: 'Registrar una cuenta bancaria' })
  @ApiCreatedResponse({ type: CuentaBancoResponseDto, description: 'Cuenta creada' })
  crearBanco(
    @Body() dto: CrearCuentaBancoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearBanco(dto, usuario);
  }

  @Patch('bancos-cuentas/:id')
  @ApiOperation({ summary: 'Actualizar una cuenta bancaria' })
  @ApiParam({ name: 'id', description: 'Id de la cuenta' })
  @ApiOkResponse({ type: CuentaBancoResponseDto, description: 'Cuenta actualizada' })
  actualizarBanco(
    @Param('id') id: string,
    @Body() dto: ActualizarCuentaBancoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.actualizarBanco(id, dto, usuario);
  }

  @Delete('bancos-cuentas/:id')
  @ApiOperation({ summary: 'Eliminar una cuenta bancaria' })
  @ApiParam({ name: 'id', description: 'Id de la cuenta' })
  @ApiOkResponse({ description: 'Cuenta eliminada' })
  eliminarBanco(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('bancos-cuentas', id, usuario);
  }

  // ── créditos ─────────────────────────────────────────────────────────────
  @Get('creditos')
  @ApiOperation({ summary: 'Listar créditos del workspace' })
  @ApiOkResponse({ type: [CreditoResponseDto], description: 'Créditos registrados' })
  listarCreditos(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarCreditos(usuario);
  }

  @Post('creditos')
  @ApiOperation({ summary: 'Registrar un crédito' })
  @ApiCreatedResponse({ type: CreditoResponseDto, description: 'Crédito creado' })
  crearCredito(@Body() dto: CrearCreditoRequestDto, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.crearCredito(dto, usuario);
  }

  @Post('creditos/:id/pagar')
  @ApiOperation({ summary: 'Marcar un crédito como pagado' })
  @ApiParam({ name: 'id', description: 'Id del crédito' })
  @ApiCreatedResponse({ type: CreditoResponseDto, description: 'Crédito pagado' })
  pagarCredito(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.pagarCredito(id, usuario);
  }

  @Delete('creditos/:id')
  @ApiOperation({ summary: 'Eliminar un crédito' })
  @ApiParam({ name: 'id', description: 'Id del crédito' })
  @ApiOkResponse({ description: 'Crédito eliminado' })
  eliminarCredito(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('creditos', id, usuario);
  }

  // ── gastos ───────────────────────────────────────────────────────────────
  @Get('gastos')
  @ApiOperation({ summary: 'Listar gastos de la empresa' })
  @ApiOkResponse({ type: [GastoResponseDto], description: 'Gastos registrados' })
  listarGastos(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarGastos(usuario);
  }

  @Post('gastos')
  @ApiOperation({ summary: 'Registrar un gasto' })
  @ApiCreatedResponse({ type: GastoResponseDto, description: 'Gasto creado' })
  crearGasto(@Body() dto: CrearGastoRequestDto, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.crearGasto(dto, usuario);
  }

  @Patch('gastos/:id')
  @ApiOperation({ summary: 'Actualizar un gasto' })
  @ApiParam({ name: 'id', description: 'Id del gasto' })
  @ApiOkResponse({ type: GastoResponseDto, description: 'Gasto actualizado' })
  actualizarGasto(
    @Param('id') id: string,
    @Body() dto: ActualizarGastoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.actualizarGasto(id, dto, usuario);
  }

  @Post('gastos/:id/pagar')
  @ApiOperation({ summary: 'Marcar un gasto como pagado' })
  @ApiParam({ name: 'id', description: 'Id del gasto' })
  @ApiCreatedResponse({ type: GastoResponseDto, description: 'Gasto pagado' })
  pagarGasto(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.actualizar('gastos', id, { status: 'pagado' }, usuario);
  }

  @Post('gastos/:id/cancelar')
  @ApiOperation({ summary: 'Cancelar un gasto' })
  @ApiParam({ name: 'id', description: 'Id del gasto' })
  @ApiCreatedResponse({ type: GastoResponseDto, description: 'Gasto cancelado' })
  cancelarGasto(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.actualizar('gastos', id, { status: 'cancelado' }, usuario);
  }

  @Delete('gastos/:id')
  @ApiOperation({ summary: 'Eliminar un gasto' })
  @ApiParam({ name: 'id', description: 'Id del gasto' })
  @ApiOkResponse({ description: 'Gasto eliminado' })
  eliminarGasto(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('gastos', id, usuario);
  }

  // ── cuentas por pagar ────────────────────────────────────────────────────
  @Get('cuentas-por-pagar')
  @ApiOperation({ summary: 'Listar cuentas por pagar del workspace' })
  @ApiOkResponse({ type: [CuentaPorPagarResponseDto], description: 'Cuentas por pagar' })
  listarCuentasPorPagar(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarCuentasPorPagar(usuario);
  }

  @Post('cuentas-por-pagar')
  @ApiOperation({ summary: 'Registrar una cuenta por pagar' })
  @ApiCreatedResponse({ type: CuentaPorPagarResponseDto, description: 'Cuenta creada' })
  crearCuentaPorPagar(
    @Body() dto: CrearCuentaPorPagarRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearCuentaPorPagar(dto, usuario);
  }

  @Post('cuentas-por-pagar/:id/pagar')
  @ApiOperation({ summary: 'Marcar una cuenta por pagar como pagada' })
  @ApiParam({ name: 'id', description: 'Id de la cuenta' })
  @ApiCreatedResponse({ type: CuentaPorPagarResponseDto, description: 'Cuenta pagada' })
  pagarCuentaPorPagar(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.pagarCuentaPorPagar(id, usuario);
  }

  @Delete('cuentas-por-pagar/:id')
  @ApiOperation({ summary: 'Eliminar una cuenta por pagar' })
  @ApiParam({ name: 'id', description: 'Id de la cuenta' })
  @ApiOkResponse({ description: 'Cuenta eliminada' })
  eliminarCuentaPorPagar(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('cuentas-por-pagar', id, usuario);
  }

  // ── proveedores ──────────────────────────────────────────────────────────
  @Get('proveedores')
  @ApiOperation({ summary: 'Listar proveedores de la empresa' })
  @ApiOkResponse({ type: [ProveedorResponseDto], description: 'Proveedores' })
  listarProveedores(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarProveedores(usuario);
  }

  @Post('proveedores')
  @ApiOperation({ summary: 'Registrar un proveedor' })
  @ApiCreatedResponse({ type: ProveedorResponseDto, description: 'Proveedor creado' })
  crearProveedor(
    @Body() dto: CrearProveedorRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearProveedor(dto, usuario);
  }

  @Delete('proveedores/:id')
  @ApiOperation({ summary: 'Eliminar un proveedor' })
  @ApiParam({ name: 'id', description: 'Id del proveedor' })
  @ApiOkResponse({ description: 'Proveedor eliminado' })
  eliminarProveedor(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('proveedores', id, usuario);
  }

  // ── compras ──────────────────────────────────────────────────────────────
  @Get('compras')
  @ApiOperation({ summary: 'Listar compras de la empresa' })
  @ApiOkResponse({ type: [CompraResponseDto], description: 'Compras registradas' })
  listarCompras(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarCompras(usuario);
  }

  @Post('compras')
  @ApiOperation({ summary: 'Registrar una compra por líneas de producto' })
  @ApiCreatedResponse({ type: CompraResponseDto, description: 'Compra creada' })
  crearCompra(@Body() dto: CrearCompraRequestDto, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.crearCompra(dto, usuario);
  }

  @Post('compras/directas')
  @ApiOperation({ summary: 'Registrar una compra directa (gasto no inventariable)' })
  @ApiCreatedResponse({ type: CompraResponseDto, description: 'Compra directa creada' })
  crearCompraDirecta(
    @Body() dto: CrearCompraDirectaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearCompraDirecta(dto, usuario);
  }

  @Post('compras/producto-nuevo')
  @ApiOperation({ summary: 'Registrar la compra de un producto nuevo' })
  @ApiCreatedResponse({ type: CompraResponseDto, description: 'Compra creada' })
  crearCompraProductoNuevo(
    @Body() dto: CrearCompraProductoNuevoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearCompraProductoNuevo(dto, usuario);
  }

  @Post('compras/:id/anular')
  @ApiOperation({ summary: 'Anular una compra' })
  @ApiParam({ name: 'id', description: 'Id de la compra' })
  @ApiCreatedResponse({ type: CompraResponseDto, description: 'Compra anulada' })
  anularCompra(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.anularCompra(id, usuario);
  }

  @Delete('compras/:id')
  @ApiOperation({ summary: 'Eliminar una compra' })
  @ApiParam({ name: 'id', description: 'Id de la compra' })
  @ApiOkResponse({ description: 'Compra eliminada' })
  eliminarCompra(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('compras', id, usuario);
  }

  // ── órdenes de compra ────────────────────────────────────────────────────
  @Get('ordenes-compra')
  @ApiOperation({ summary: 'Listar órdenes de compra' })
  @ApiOkResponse({ type: [OrdenCompraResponseDto], description: 'Órdenes de compra' })
  listarOrdenesCompra(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarOrdenesCompra(usuario);
  }

  @Post('ordenes-compra')
  @ApiOperation({ summary: 'Crear una orden de compra' })
  @ApiCreatedResponse({ type: OrdenCompraResponseDto, description: 'Orden creada' })
  crearOrdenCompra(
    @Body() dto: CrearOrdenCompraRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearOrdenCompra(dto, usuario);
  }

  @Post('ordenes-compra/:id/convertir')
  @ApiOperation({ summary: 'Convertir una orden pendiente en compra' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  @ApiCreatedResponse({ type: OrdenCompraResponseDto, description: 'Orden convertida' })
  convertirOrdenCompra(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.convertirOrdenCompra(id, usuario);
  }

  @Post('ordenes-compra/:id/anular')
  @ApiOperation({ summary: 'Anular una orden de compra' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  @ApiCreatedResponse({ type: OrdenCompraResponseDto, description: 'Orden anulada' })
  anularOrdenCompra(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.anularOrdenCompra(id, usuario);
  }

  // ── configuración de documentos ──────────────────────────────────────────
  @Get('configuracion-documentos')
  @ApiOperation({ summary: 'Obtener las preferencias de documentos imprimibles' })
  @ApiOkResponse({ type: ConfiguracionDocumentosResponseDto, description: 'Configuración vigente' })
  obtenerConfiguracionDocumentos(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.obtenerConfiguracionDocumentos(usuario);
  }

  @Put('configuracion-documentos')
  @ApiOperation({ summary: 'Guardar las preferencias de documentos imprimibles' })
  @ApiOkResponse({ type: ConfiguracionDocumentosResponseDto, description: 'Configuración guardada' })
  actualizarConfiguracionDocumentos(
    @Body() dto: ActualizarConfiguracionDocumentosRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.actualizarConfiguracionDocumentos(dto, usuario);
  }

  // ── caja: métodos de pago ────────────────────────────────────────────────
  @Get('caja/metodos-pago')
  @ApiOperation({ summary: 'Listar métodos de pago de la caja' })
  @ApiOkResponse({ type: [MetodoPagoResponseDto], description: 'Métodos de pago' })
  listarMetodosPago(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarMetodosPago(usuario);
  }

  @Post('caja/metodos-pago')
  @ApiOperation({ summary: 'Agregar un método de pago' })
  @ApiCreatedResponse({ type: MetodoPagoResponseDto, description: 'Método creado' })
  crearMetodoPago(
    @Body() dto: CrearMetodoPagoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearMetodoPago(dto, usuario);
  }

  @Patch('caja/metodos-pago/:id')
  @ApiOperation({ summary: 'Actualizar un método de pago' })
  @ApiParam({ name: 'id', description: 'Id del método' })
  @ApiOkResponse({ type: MetodoPagoResponseDto, description: 'Método actualizado' })
  actualizarMetodoPago(
    @Param('id') id: string,
    @Body() dto: ActualizarMetodoPagoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.actualizarMetodoPago(id, dto, usuario);
  }

  @Delete('caja/metodos-pago/:id')
  @ApiOperation({ summary: 'Eliminar un método de pago sin movimientos' })
  @ApiParam({ name: 'id', description: 'Id del método' })
  @ApiOkResponse({ description: 'Método eliminado' })
  eliminarMetodoPago(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminarMetodoPago(id, usuario);
  }

  // ── caja: movimientos ────────────────────────────────────────────────────
  @Get('caja/movimientos')
  @ApiOperation({ summary: 'Listar movimientos de caja' })
  @ApiOkResponse({ type: [MovimientoCajaResponseDto], description: 'Movimientos de caja' })
  listarMovimientosCaja(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarMovimientosCaja(usuario);
  }

  @Post('caja/movimientos')
  @ApiOperation({ summary: 'Registrar un movimiento de caja' })
  @ApiCreatedResponse({ type: MovimientoCajaResponseDto, description: 'Movimiento creado' })
  crearMovimientoCaja(
    @Body() dto: CrearMovimientoCajaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearMovimientoCaja(dto, usuario);
  }

  @Post('caja/movimientos/:id/anular')
  @ApiOperation({ summary: 'Anular un movimiento de caja' })
  @ApiParam({ name: 'id', description: 'Id del movimiento' })
  @ApiCreatedResponse({ type: MovimientoCajaResponseDto, description: 'Movimiento anulado' })
  anularMovimientoCaja(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.anularMovimientoCaja(id, usuario);
  }

  @Delete('caja/movimientos/:id')
  @ApiOperation({ summary: 'Eliminar un movimiento de caja' })
  @ApiParam({ name: 'id', description: 'Id del movimiento' })
  @ApiOkResponse({ description: 'Movimiento eliminado' })
  eliminarMovimientoCaja(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('caja-movimientos', id, usuario);
  }

  // ── caja: cierres ────────────────────────────────────────────────────────
  @Get('caja/cierres')
  @ApiOperation({ summary: 'Listar cierres de caja' })
  @ApiOkResponse({ type: [CierreCajaResponseDto], description: 'Cierres registrados' })
  listarCierres(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarCierres(usuario);
  }

  @Get('caja/cierres/preview')
  @ApiOperation({ summary: 'Previsualizar el corte del día' })
  @ApiQuery({ name: 'fecha', required: false, description: 'Fecha local yyyy-mm-dd' })
  @ApiOkResponse({ type: CierreCajaPreviewResponseDto, description: 'Corte calculado' })
  previewCierre(@UsuarioActual() usuario: UsuarioContexto, @Query('fecha') fecha?: string) {
    return this.comercio.previewCierre(usuario, fecha);
  }

  @Post('caja/cierres')
  @ApiOperation({ summary: 'Registrar el cierre diario de caja' })
  @ApiCreatedResponse({ type: CierreCajaResponseDto, description: 'Cierre registrado' })
  crearCierre(
    @Body() dto: CrearCierreCajaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearCierre(dto, usuario);
  }

  // ── caja: caja chica ─────────────────────────────────────────────────────
  @Get('caja/chica/fondo')
  @ApiOperation({ summary: 'Obtener el fondo de caja chica' })
  @ApiOkResponse({ type: FondoCajaChicaResponseDto, description: 'Fondo vigente' })
  obtenerFondoCajaChica(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.obtenerFondoCajaChica(usuario);
  }

  @Put('caja/chica/fondo')
  @ApiOperation({ summary: 'Actualizar el fondo de caja chica' })
  @ApiOkResponse({ type: FondoCajaChicaResponseDto, description: 'Fondo actualizado' })
  guardarFondoCajaChica(
    @Body() dto: CrearFondoCajaChicaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.guardarFondoCajaChica(dto, usuario);
  }

  @Get('caja/chica/movimientos')
  @ApiOperation({ summary: 'Listar movimientos de caja chica' })
  @ApiOkResponse({ type: [MovimientoCajaChicaResponseDto], description: 'Movimientos de caja chica' })
  listarMovimientosCajaChica(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarMovimientosCajaChica(usuario);
  }

  @Post('caja/chica/movimientos')
  @ApiOperation({ summary: 'Registrar un movimiento de caja chica' })
  @ApiCreatedResponse({ type: MovimientoCajaChicaResponseDto, description: 'Movimiento creado' })
  crearMovimientoCajaChica(
    @Body() dto: CrearMovimientoCajaChicaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearMovimientoCajaChica(dto, usuario);
  }

  // ── caja: transferencias ─────────────────────────────────────────────────
  @Get('caja/transferencias')
  @ApiOperation({ summary: 'Listar transferencias internas de caja' })
  @ApiOkResponse({ type: [TransferenciaCajaResponseDto], description: 'Transferencias' })
  listarTransferencias(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarTransferencias(usuario);
  }

  @Post('caja/transferencias')
  @ApiOperation({ summary: 'Registrar una transferencia entre caja y bancos' })
  @ApiCreatedResponse({ type: TransferenciaCajaResponseDto, description: 'Transferencia creada' })
  crearTransferencia(
    @Body() dto: CrearTransferenciaCajaRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearTransferencia(dto, usuario);
  }

  @Post('caja/transferencias/:id/anular')
  @ApiOperation({ summary: 'Anular una transferencia interna' })
  @ApiParam({ name: 'id', description: 'Id de la transferencia' })
  @ApiCreatedResponse({ type: TransferenciaCajaResponseDto, description: 'Transferencia anulada' })
  anularTransferencia(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.anularTransferencia(id, usuario);
  }

  // ── clientes ─────────────────────────────────────────────────────────────
  @Get('clientes')
  @ApiOperation({ summary: 'Listar la cartera de clientes del workspace' })
  @ApiOkResponse({ type: [ClienteComercioResponseDto], description: 'Clientes registrados' })
  listarClientes(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarClientes(usuario);
  }

  @Post('clientes')
  @ApiOperation({ summary: 'Registrar un cliente manual' })
  @ApiCreatedResponse({ type: ClienteComercioResponseDto, description: 'Cliente creado' })
  crearCliente(@Body() dto: CrearClienteComercioRequestDto, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.crearCliente(dto, usuario);
  }

  @Post('clientes/importar')
  @ApiOperation({ summary: 'Importar una cartera de clientes' })
  @ApiCreatedResponse({ type: ImportarClientesResponseDto, description: 'Resultado de la importación' })
  importarClientes(
    @Body() dto: ImportarClientesRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.importarClientes(dto, usuario);
  }

  @Patch('clientes/:id')
  @ApiOperation({ summary: 'Actualizar un cliente' })
  @ApiParam({ name: 'id', description: 'Id del cliente' })
  @ApiOkResponse({ type: ClienteComercioResponseDto, description: 'Cliente actualizado' })
  actualizarCliente(
    @Param('id') id: string,
    @Body() dto: ActualizarClienteComercioRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.actualizarCliente(id, dto, usuario);
  }

  @Delete('clientes/:id')
  @ApiOperation({ summary: 'Eliminar un cliente' })
  @ApiParam({ name: 'id', description: 'Id del cliente' })
  @ApiOkResponse({ description: 'Cliente eliminado' })
  eliminarCliente(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('clientes', id, usuario);
  }

  // ── códigos de barras ────────────────────────────────────────────────────
  @Get('codigos-barras')
  @ApiOperation({ summary: 'Listar etiquetas de códigos de barras' })
  @ApiOkResponse({ type: [CodigoBarrasResponseDto], description: 'Etiquetas registradas' })
  listarCodigosBarras(@UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.listarCodigosBarras(usuario);
  }

  @Post('codigos-barras')
  @ApiOperation({ summary: 'Generar una etiqueta de código de barras' })
  @ApiCreatedResponse({ type: CodigoBarrasResponseDto, description: 'Etiqueta creada' })
  crearCodigoBarras(
    @Body() dto: CrearCodigoBarrasRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.comercio.crearCodigoBarras(dto, usuario);
  }

  @Delete('codigos-barras/:id')
  @ApiOperation({ summary: 'Eliminar una etiqueta de código de barras' })
  @ApiParam({ name: 'id', description: 'Id de la etiqueta' })
  @ApiOkResponse({ description: 'Etiqueta eliminada' })
  eliminarCodigoBarras(@Param('id') id: string, @UsuarioActual() usuario: UsuarioContexto) {
    return this.comercio.eliminar('codigos-barras', id, usuario);
  }
}
