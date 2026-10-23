import { Body, Controller, Delete, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiTags } from '@nestjs/swagger';
import {
  UsuarioActual,
  UsuarioContexto,
  Roles,
  ROLES,
  ROLES_LOGISTICA,
  DomainError,
} from '@core/shared';
import { FieldService } from './field.service';
import {
  CrearPersonalRequestDto,
  ActualizarPersonalRequestDto,
  CrearClienteRequestDto,
  ActualizarClienteRequestDto,
  CrearVehiculoRequestDto,
  ActualizarVehiculoRequestDto,
  CrearRutaRequestDto,
  ActualizarRutaRequestDto,
  ParadaRequestDto,
  AsignarRutaRequestDto,
  CrearPedidoRequestDto,
  ActualizarPedidoRequestDto,
  CambiarEstadoPedidoRequestDto,
  CrearAsistenciaRequestDto,
  CrearIncidenciaRequestDto,
  ActualizarIncidenciaRequestDto,
  CrearVisitaRequestDto,
  GuardarCumplimientoRequestDto,
  UbicacionRequestDto,
  SyncRequestDto,
  TrackingBulkRequestDto,
} from './field.dtos';
import {
  AsistenciaResponseDto,
  ClienteResponseDto,
  CumplimientoResponseDto,
  IncidenciaResponseDto,
  ParadaResponseDto,
  PedidoResponseDto,
  PersonalResponseDto,
  RutaResponseDto,
  SyncResultadoResponseDto,
  TrackingResponseDto,
  UbicacionResponseDto,
  VehiculoResponseDto,
  VisitaResponseDto,
} from './field.response.dto';

/** El operador global conserva acceso a las rutas de logística de campo (ADR-25). */
const ROLES_CAMPO = [ROLES.SUPER_ADMIN, ...ROLES_LOGISTICA];

@ApiTags('Field (logística de campo)')
@ApiBearerAuth()
@Controller('api/v1/field')
export class FieldController {
  constructor(private readonly field: FieldService) {}

  private tenant(usuario?: UsuarioContexto): string {
    if (!usuario?.tenant_id) {
      throw new DomainError('TENANT_REQUERIDO', 'El contexto no tiene tenant (x-tenant).');
    }
    return usuario.tenant_id;
  }

  // ── personal ───────────────────────────────────────────────────────────
  @Get('personal')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar personal' })
  @ApiOkResponse({ type: [PersonalResponseDto], description: 'Personal del tenant' })
  listarPersonal(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarPersonal(this.tenant(u));
  }
  @Get('personal/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Obtener personal por id' })
  @ApiOkResponse({ type: PersonalResponseDto, description: 'Personal encontrado' })
  obtenerPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerPersonal(this.tenant(u), id);
  }
  @Post('personal')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Crear personal' })
  @ApiCreatedResponse({ type: PersonalResponseDto, description: 'Personal creado' })
  crearPersonal(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearPersonalRequestDto) {
    return this.field.crearPersonal(this.tenant(u), dto);
  }
  @Patch('personal/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Actualizar personal' })
  @ApiOkResponse({ type: PersonalResponseDto, description: 'Personal actualizado' })
  actualizarPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarPersonalRequestDto) {
    return this.field.actualizarPersonal(this.tenant(u), id, dto);
  }
  @Delete('personal/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar personal' })
  @ApiOkResponse({ description: 'Personal eliminado' })
  eliminarPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarPersonal(this.tenant(u), id);
  }
  @Get('personal/:id/ubicacion')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Ubicación del personal' })
  @ApiOkResponse({ type: UbicacionResponseDto, description: 'Ubicación del personal' })
  ubicacionPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.ubicacionPersonal(this.tenant(u), id);
  }
  @Patch('personal/:id/ubicacion')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Actualizar ubicación del personal' })
  @ApiOkResponse({ type: PersonalResponseDto, description: 'Ubicación actualizada' })
  actualizarUbicacion(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: UbicacionRequestDto) {
    return this.field.actualizarUbicacion(this.tenant(u), id, dto);
  }

  // ── clientes ───────────────────────────────────────────────────────────
  @Get('clientes')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar clientes de campo' })
  @ApiOkResponse({ type: [ClienteResponseDto], description: 'Clientes de campo' })
  listarClientes(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarClientes(this.tenant(u));
  }
  @Get('clientes/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Obtener cliente por id' })
  @ApiOkResponse({ type: ClienteResponseDto, description: 'Cliente encontrado' })
  obtenerCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerCliente(this.tenant(u), id);
  }
  @Post('clientes')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Crear cliente de campo' })
  @ApiCreatedResponse({ type: ClienteResponseDto, description: 'Cliente creado' })
  crearCliente(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearClienteRequestDto) {
    return this.field.crearCliente(this.tenant(u), dto);
  }
  @Patch('clientes/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Actualizar cliente de campo' })
  @ApiOkResponse({ type: ClienteResponseDto, description: 'Cliente actualizado' })
  actualizarCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarClienteRequestDto) {
    return this.field.actualizarCliente(this.tenant(u), id, dto);
  }
  @Delete('clientes/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar cliente de campo' })
  @ApiOkResponse({ description: 'Cliente eliminado' })
  eliminarCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarCliente(this.tenant(u), id);
  }

  // ── vehiculos ──────────────────────────────────────────────────────────
  @Get('vehiculos')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar vehículos' })
  @ApiOkResponse({ type: [VehiculoResponseDto], description: 'Vehículos del tenant' })
  listarVehiculos(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarVehiculos(this.tenant(u));
  }
  @Get('vehiculos/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Obtener vehículo por id' })
  @ApiOkResponse({ type: VehiculoResponseDto, description: 'Vehículo encontrado' })
  obtenerVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerVehiculo(this.tenant(u), id);
  }
  @Post('vehiculos')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Crear vehículo' })
  @ApiCreatedResponse({ type: VehiculoResponseDto, description: 'Vehículo creado' })
  crearVehiculo(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearVehiculoRequestDto) {
    return this.field.crearVehiculo(this.tenant(u), dto);
  }
  @Patch('vehiculos/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Actualizar vehículo' })
  @ApiOkResponse({ type: VehiculoResponseDto, description: 'Vehículo actualizado' })
  actualizarVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarVehiculoRequestDto) {
    return this.field.actualizarVehiculo(this.tenant(u), id, dto);
  }
  @Delete('vehiculos/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar vehículo' })
  @ApiOkResponse({ description: 'Vehículo eliminado' })
  eliminarVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarVehiculo(this.tenant(u), id);
  }

  // ── rutas ──────────────────────────────────────────────────────────────
  @Get('rutas')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar rutas' })
  @ApiOkResponse({ type: [RutaResponseDto], description: 'Rutas del tenant' })
  listarRutas(@UsuarioActual() u: UsuarioContexto, @Query('personalId') personalId?: string) {
    return this.field.listarRutas(this.tenant(u), personalId);
  }
  @Get('rutas/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Obtener ruta por id' })
  @ApiOkResponse({ type: RutaResponseDto, description: 'Ruta encontrada' })
  obtenerRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerRuta(this.tenant(u), id);
  }
  @Post('rutas')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Crear ruta' })
  @ApiCreatedResponse({ type: RutaResponseDto, description: 'Ruta creada' })
  crearRuta(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearRutaRequestDto) {
    return this.field.crearRuta(this.tenant(u), dto);
  }
  @Patch('rutas/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Actualizar ruta' })
  @ApiOkResponse({ type: RutaResponseDto, description: 'Ruta actualizada' })
  actualizarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarRutaRequestDto) {
    return this.field.actualizarRuta(this.tenant(u), id, dto);
  }
  @Delete('rutas/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar ruta' })
  @ApiOkResponse({ description: 'Ruta eliminada' })
  eliminarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarRuta(this.tenant(u), id);
  }
  @Post('rutas/:id/paradas')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Agregar parada a la ruta' })
  @ApiCreatedResponse({ type: ParadaResponseDto, description: 'Parada creada' })
  agregarParada(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ParadaRequestDto) {
    return this.field.agregarParada(this.tenant(u), id, dto);
  }
  @Patch('rutas/:id/paradas/:paradaId')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Actualizar parada de la ruta' })
  @ApiOkResponse({ type: ParadaResponseDto, description: 'Parada actualizada' })
  actualizarParada(
    @UsuarioActual() u: UsuarioContexto,
    @Param('id') id: string,
    @Param('paradaId') paradaId: string,
    @Body() dto: ParadaRequestDto,
  ) {
    return this.field.actualizarParada(this.tenant(u), id, paradaId, dto);
  }
  @Post('rutas/:id/asignar')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Asignar personal/vehículo a la ruta' })
  @ApiCreatedResponse({ type: RutaResponseDto, description: 'Ruta asignada' })
  asignarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: AsignarRutaRequestDto) {
    return this.field.asignarRuta(this.tenant(u), id, dto);
  }

  // ── pedidos ────────────────────────────────────────────────────────────
  @Get('pedidos')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar pedidos de campo' })
  @ApiOkResponse({ type: [PedidoResponseDto], description: 'Pedidos de campo' })
  listarPedidos(
    @UsuarioActual() u: UsuarioContexto,
    @Query('estado') estado?: string,
    @Query('rutaId') rutaId?: string,
    @Query('clienteId') clienteId?: string,
  ) {
    return this.field.listarPedidos(this.tenant(u), { estado, rutaId, clienteId });
  }
  @Get('pedidos/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Obtener pedido por id' })
  @ApiOkResponse({ type: PedidoResponseDto, description: 'Pedido encontrado' })
  obtenerPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerPedido(this.tenant(u), id);
  }
  @Post('pedidos')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Crear pedido de campo' })
  @ApiCreatedResponse({ type: PedidoResponseDto, description: 'Pedido creado' })
  crearPedido(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearPedidoRequestDto) {
    return this.field.crearPedido(this.tenant(u), dto);
  }
  @Patch('pedidos/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Actualizar pedido de campo' })
  @ApiOkResponse({ type: PedidoResponseDto, description: 'Pedido actualizado' })
  actualizarPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarPedidoRequestDto) {
    return this.field.actualizarPedido(this.tenant(u), id, dto);
  }
  @Delete('pedidos/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar pedido de campo' })
  @ApiOkResponse({ description: 'Pedido eliminado' })
  eliminarPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarPedido(this.tenant(u), id);
  }
  @Patch('pedidos/:id/estado')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Cambiar estado del pedido' })
  @ApiOkResponse({ type: PedidoResponseDto, description: 'Pedido actualizado' })
  cambiarEstadoPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: CambiarEstadoPedidoRequestDto) {
    return this.field.cambiarEstadoPedido(this.tenant(u), id, dto);
  }

  // ── asistencia ─────────────────────────────────────────────────────────
  @Get('asistencia')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar asistencia' })
  @ApiOkResponse({ type: [AsistenciaResponseDto], description: 'Registros de asistencia' })
  listarAsistencia(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('fecha') fecha?: string,
  ) {
    return this.field.listarAsistencia(this.tenant(u), { personalId, fecha });
  }
  @Post('asistencia')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Registrar asistencia' })
  @ApiCreatedResponse({ type: AsistenciaResponseDto, description: 'Asistencia registrada' })
  registrarAsistencia(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearAsistenciaRequestDto) {
    return this.field.registrarAsistencia(this.tenant(u), dto);
  }

  // ── incidencias ───────────────────────────────────────────────────────
  @Get('incidencias')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar incidencias' })
  @ApiOkResponse({ type: [IncidenciaResponseDto], description: 'Incidencias' })
  listarIncidencias(
    @UsuarioActual() u: UsuarioContexto,
    @Query('estado') estado?: string,
    @Query('rutaId') rutaId?: string,
  ) {
    return this.field.listarIncidencias(this.tenant(u), { estado, rutaId });
  }
  @Get('incidencias/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Obtener incidencia por id' })
  @ApiOkResponse({ type: IncidenciaResponseDto, description: 'Incidencia encontrada' })
  obtenerIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerIncidencia(this.tenant(u), id);
  }
  @Post('incidencias')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Crear incidencia' })
  @ApiCreatedResponse({ type: IncidenciaResponseDto, description: 'Incidencia creada' })
  crearIncidencia(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearIncidenciaRequestDto) {
    return this.field.crearIncidencia(this.tenant(u), dto);
  }
  @Patch('incidencias/:id')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Actualizar incidencia' })
  @ApiOkResponse({ type: IncidenciaResponseDto, description: 'Incidencia actualizada' })
  actualizarIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarIncidenciaRequestDto) {
    return this.field.actualizarIncidencia(this.tenant(u), id, dto);
  }
  @Delete('incidencias/:id')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar incidencia' })
  @ApiOkResponse({ description: 'Incidencia eliminada' })
  eliminarIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarIncidencia(this.tenant(u), id);
  }

  // ── tracking (GPS) ────────────────────────────────────────────────────
  @Get('tracking')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar tracking GPS' })
  @ApiOkResponse({ type: [TrackingResponseDto], description: 'Registros de tracking' })
  listarTracking(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
  ) {
    return this.field.listarTracking(this.tenant(u), { personalId, desde, hasta });
  }
  @Post('tracking')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Registrar tracking GPS (lote)' })
  @ApiCreatedResponse({ type: [TrackingResponseDto], description: 'Tracking registrado' })
  registrarTracking(@UsuarioActual() u: UsuarioContexto, @Body() dto: TrackingBulkRequestDto) {
    return this.field.registrarTracking(this.tenant(u), dto.registros);
  }

  // ── visitas (telemetria) ──────────────────────────────────────────────
  @Get('visitas')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Listar visitas (telemetría)' })
  @ApiOkResponse({ type: [VisitaResponseDto], description: 'Visitas registradas' })
  listarVisitas(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('fecha') fecha?: string,
  ) {
    return this.field.listarVisitas(this.tenant(u), { personalId, fecha });
  }
  @Post('visitas')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Registrar visita (telemetría)' })
  @ApiCreatedResponse({ type: VisitaResponseDto, description: 'Visita registrada' })
  registrarVisita(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearVisitaRequestDto) {
    return this.field.registrarVisita(this.tenant(u), dto);
  }

  // ── cumplimiento ──────────────────────────────────────────────────────
  @Get('cumplimiento/:rutaId/:fecha')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Obtener cumplimiento de ruta/fecha' })
  @ApiOkResponse({ type: CumplimientoResponseDto, description: 'Cumplimiento de la ruta' })
  obtenerCumplimiento(@UsuarioActual() u: UsuarioContexto, @Param('rutaId') rutaId: string, @Param('fecha') fecha: string) {
    return this.field.obtenerCumplimiento(this.tenant(u), rutaId, fecha);
  }
  @Post('cumplimiento')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Guardar cumplimiento de ruta' })
  @ApiCreatedResponse({ type: CumplimientoResponseDto, description: 'Cumplimiento guardado' })
  guardarCumplimiento(@UsuarioActual() u: UsuarioContexto, @Body() dto: GuardarCumplimientoRequestDto) {
    return this.field.guardarCumplimiento(this.tenant(u), dto);
  }

  // ── sync offline (app-test) ───────────────────────────────────────────
  @Post('sync')
  @Roles(...ROLES_CAMPO)
  @ApiOperation({ summary: 'Sincronizar operaciones offline' })
  @ApiCreatedResponse({ type: [SyncResultadoResponseDto], description: 'Resultado de la sincronización' })
  sincronizar(@UsuarioActual() u: UsuarioContexto, @Body() dto: SyncRequestDto) {
    return this.field.sincronizar(this.tenant(u), dto.operaciones);
  }
}
