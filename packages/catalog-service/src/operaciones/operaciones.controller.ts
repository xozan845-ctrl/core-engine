import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  Param,
  Patch,
  Post,
  Put,
  Query,
} from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiNoContentResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiQuery,
  ApiTags,
} from '@nestjs/swagger';
import { DomainError, NotFoundError, ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import {
  ActualizarComboRequestDto,
  ActualizarConfiguracionInventarioRequestDto,
  ActualizarPromocionRequestDto,
  CrearComboRequestDto,
  CrearMovimientoInventarioRequestDto,
  CrearPromocionRequestDto,
  ImportarProductosRequestDto,
  ListarCombosRequestDto,
  ListarMovimientosRequestDto,
  ListarPromocionesRequestDto,
} from './operaciones.dtos';
import {
  OperacionesService,
} from './operaciones.service';
import {
  ComboResponseDto,
  ConfiguracionInventarioResponseDto,
  EscaneoResponseDto,
  ImportarProductosResponseDto,
  MovimientoInventarioResponseDto,
  PaginaCombosResponseDto,
  PaginaMovimientosResponseDto,
  PaginaPromocionesResponseDto,
  PromocionResponseDto,
} from './operaciones.response.dto';

/**
 * Operaciones avanzadas del catálogo (R-PS-04). La empresa define aquí la
 * configuración de inventario, importa productos, consulta códigos, administra
 * combos/promociones y registra movimientos o traslados.
 */
@ApiTags('Catálogo · Operaciones')
@ApiBearerAuth()
@Controller('api/v1/catalog')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
export class OperacionesController {
  constructor(private readonly operaciones: OperacionesService) {}

  @Get('configuracion-inventario')
  @ApiOperation({ summary: 'Obtener la configuración de inventario de la empresa' })
  @ApiQuery({ name: 'empresa_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: ConfiguracionInventarioResponseDto })
  configuracion(
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.obtenerConfiguracion(this.empresa(usuario, empresaId));
  }

  @Put('configuracion-inventario')
  @ApiOperation({ summary: 'Guardar la configuración de inventario' })
  @ApiQuery({ name: 'empresa_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: ConfiguracionInventarioResponseDto })
  guardarConfiguracion(
    @Body() dto: ActualizarConfiguracionInventarioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.guardarConfiguracion(this.empresa(usuario, empresaId), dto);
  }

  @Post('productos/importar')
  @ApiOperation({ summary: 'Importar productos desde filas validadas (máx. 200)' })
  @ApiQuery({ name: 'empresa_id', required: false, format: 'uuid' })
  @ApiCreatedResponse({ type: ImportarProductosResponseDto })
  importar(
    @Body() dto: ImportarProductosRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.importarProductos(this.empresa(usuario, empresaId), dto.filas);
  }

  @Get('escanear')
  @ApiOperation({ summary: 'Buscar un producto o servicio por código/barras' })
  @ApiQuery({ name: 'codigo', required: true })
  @ApiQuery({ name: 'empresa_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: EscaneoResponseDto })
  escanear(
    @Query('codigo') codigo: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.escanear(this.empresa(usuario, empresaId), codigo ?? '');
  }

  // ── combos ───────────────────────────────────────────────────────────────
  @Get('combos')
  @ApiOperation({ summary: 'Listar combos de la empresa' })
  @ApiOkResponse({ type: PaginaCombosResponseDto })
  listarCombos(
    @Query() query: ListarCombosRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.listarCombos(this.empresa(usuario, query.empresa_id), query);
  }

  @Post('combos')
  @ApiOperation({ summary: 'Crear un combo' })
  @ApiCreatedResponse({ type: ComboResponseDto })
  crearCombo(
    @Body() dto: CrearComboRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.crearCombo(this.empresa(usuario, empresaId), dto);
  }

  @Get('combos/:id')
  @ApiOperation({ summary: 'Detalle de un combo' })
  @ApiParam({ name: 'id' })
  @ApiOkResponse({ type: ComboResponseDto })
  async detalleCombo(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const combo = await this.operaciones.obtenerCombo(id, this.empresa(usuario, empresaId));
    if (!combo) throw new NotFoundError('Combo', id);
    return combo;
  }

  @Patch('combos/:id')
  @ApiOperation({ summary: 'Actualizar un combo' })
  @ApiParam({ name: 'id' })
  @ApiOkResponse({ type: ComboResponseDto })
  actualizarCombo(
    @Param('id') id: string,
    @Body() dto: ActualizarComboRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.actualizarCombo(id, this.empresa(usuario, empresaId), dto);
  }

  @Delete('combos/:id')
  @HttpCode(204)
  @ApiOperation({ summary: 'Dar de baja un combo' })
  @ApiParam({ name: 'id' })
  @ApiNoContentResponse({ description: 'Combo dado de baja' })
  async eliminarCombo(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    await this.operaciones.eliminarCombo(id, this.empresa(usuario, empresaId));
  }

  // ── promociones ──────────────────────────────────────────────────────────
  @Get('promociones')
  @ApiOperation({ summary: 'Listar promociones de la empresa' })
  @ApiOkResponse({ type: PaginaPromocionesResponseDto })
  listarPromociones(
    @Query() query: ListarPromocionesRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.listarPromociones(this.empresa(usuario, query.empresa_id), query);
  }

  @Post('promociones')
  @ApiOperation({ summary: 'Crear una promoción' })
  @ApiCreatedResponse({ type: PromocionResponseDto })
  crearPromocion(
    @Body() dto: CrearPromocionRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.crearPromocion(this.empresa(usuario, empresaId), dto);
  }

  @Get('promociones/:id')
  @ApiOperation({ summary: 'Detalle de una promoción' })
  @ApiParam({ name: 'id' })
  @ApiOkResponse({ type: PromocionResponseDto })
  async detallePromocion(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    const promocion = await this.operaciones.obtenerPromocion(
      id,
      this.empresa(usuario, empresaId),
    );
    if (!promocion) throw new NotFoundError('Promoción', id);
    return promocion;
  }

  @Patch('promociones/:id')
  @ApiOperation({ summary: 'Actualizar una promoción' })
  @ApiParam({ name: 'id' })
  @ApiOkResponse({ type: PromocionResponseDto })
  actualizarPromocion(
    @Param('id') id: string,
    @Body() dto: ActualizarPromocionRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.actualizarPromocion(id, this.empresa(usuario, empresaId), dto);
  }

  @Delete('promociones/:id')
  @HttpCode(204)
  @ApiOperation({ summary: 'Dar de baja una promoción' })
  @ApiParam({ name: 'id' })
  @ApiNoContentResponse({ description: 'Promoción dada de baja' })
  async eliminarPromocion(
    @Param('id') id: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<void> {
    await this.operaciones.eliminarPromocion(id, this.empresa(usuario, empresaId));
  }

  // ── inventario ───────────────────────────────────────────────────────────
  @Get('inventario/movimientos')
  @ApiOperation({ summary: 'Listar movimientos y traslados de inventario' })
  @ApiOkResponse({ type: PaginaMovimientosResponseDto })
  listarMovimientos(
    @Query() query: ListarMovimientosRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.listarMovimientos(this.empresa(usuario, query.empresa_id), query);
  }

  @Post('inventario/movimientos')
  @ApiOperation({ summary: 'Registrar entrada, salida, ajuste o traslado' })
  @ApiCreatedResponse({ type: MovimientoInventarioResponseDto })
  registrarMovimiento(
    @Body() dto: CrearMovimientoInventarioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.operaciones.registrarMovimiento(
      this.empresa(usuario, empresaId),
      dto,
      usuario.user_id,
    );
  }

  private empresa(usuario: UsuarioContexto, solicitada?: string): string {
    if (usuario.rol === ROLES.EMPRESARIO) {
      if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
      if (solicitada && solicitada !== usuario.tenant_id) {
        throw new DomainError(
          'EMPRESA_NO_AUTORIZADA',
          'El empresario solo puede operar el catálogo de su empresa.',
        );
      }
      return usuario.tenant_id;
    }
    const empresaId = solicitada ?? usuario.tenant_id;
    if (!empresaId) {
      throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id para operar el catálogo.');
    }
    return empresaId;
  }
}
