import { Body, Controller, Delete, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { ApiOperation, ApiTags } from '@nestjs/swagger';
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

@ApiTags('Field (logística de campo)')
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
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar personal' })
  listarPersonal(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarPersonal(this.tenant(u));
  }
  @Get('personal/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Obtener personal por id' })
  obtenerPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerPersonal(this.tenant(u), id);
  }
  @Post('personal')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Crear personal' })
  crearPersonal(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearPersonalRequestDto) {
    return this.field.crearPersonal(this.tenant(u), dto);
  }
  @Patch('personal/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Actualizar personal' })
  actualizarPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarPersonalRequestDto) {
    return this.field.actualizarPersonal(this.tenant(u), id, dto);
  }
  @Delete('personal/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar personal' })
  eliminarPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarPersonal(this.tenant(u), id);
  }
  @Get('personal/:id/ubicacion')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Ubicación del personal' })
  ubicacionPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.ubicacionPersonal(this.tenant(u), id);
  }
  @Patch('personal/:id/ubicacion')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Actualizar ubicación del personal' })
  actualizarUbicacion(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: UbicacionRequestDto) {
    return this.field.actualizarUbicacion(this.tenant(u), id, dto);
  }

  // ── clientes ───────────────────────────────────────────────────────────
  @Get('clientes')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar clientes de campo' })
  listarClientes(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarClientes(this.tenant(u));
  }
  @Get('clientes/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Obtener cliente por id' })
  obtenerCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerCliente(this.tenant(u), id);
  }
  @Post('clientes')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Crear cliente de campo' })
  crearCliente(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearClienteRequestDto) {
    return this.field.crearCliente(this.tenant(u), dto);
  }
  @Patch('clientes/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Actualizar cliente de campo' })
  actualizarCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarClienteRequestDto) {
    return this.field.actualizarCliente(this.tenant(u), id, dto);
  }
  @Delete('clientes/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar cliente de campo' })
  eliminarCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarCliente(this.tenant(u), id);
  }

  // ── vehiculos ──────────────────────────────────────────────────────────
  @Get('vehiculos')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar vehículos' })
  listarVehiculos(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarVehiculos(this.tenant(u));
  }
  @Get('vehiculos/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Obtener vehículo por id' })
  obtenerVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerVehiculo(this.tenant(u), id);
  }
  @Post('vehiculos')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Crear vehículo' })
  crearVehiculo(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearVehiculoRequestDto) {
    return this.field.crearVehiculo(this.tenant(u), dto);
  }
  @Patch('vehiculos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Actualizar vehículo' })
  actualizarVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarVehiculoRequestDto) {
    return this.field.actualizarVehiculo(this.tenant(u), id, dto);
  }
  @Delete('vehiculos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar vehículo' })
  eliminarVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarVehiculo(this.tenant(u), id);
  }

  // ── rutas ──────────────────────────────────────────────────────────────
  @Get('rutas')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar rutas' })
  listarRutas(@UsuarioActual() u: UsuarioContexto, @Query('personalId') personalId?: string) {
    return this.field.listarRutas(this.tenant(u), personalId);
  }
  @Get('rutas/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Obtener ruta por id' })
  obtenerRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerRuta(this.tenant(u), id);
  }
  @Post('rutas')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Crear ruta' })
  crearRuta(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearRutaRequestDto) {
    return this.field.crearRuta(this.tenant(u), dto);
  }
  @Patch('rutas/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Actualizar ruta' })
  actualizarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarRutaRequestDto) {
    return this.field.actualizarRuta(this.tenant(u), id, dto);
  }
  @Delete('rutas/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar ruta' })
  eliminarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarRuta(this.tenant(u), id);
  }
  @Post('rutas/:id/paradas')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Agregar parada a la ruta' })
  agregarParada(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ParadaRequestDto) {
    return this.field.agregarParada(this.tenant(u), id, dto);
  }
  @Patch('rutas/:id/paradas/:paradaId')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Actualizar parada de la ruta' })
  actualizarParada(
    @UsuarioActual() u: UsuarioContexto,
    @Param('id') id: string,
    @Param('paradaId') paradaId: string,
    @Body() dto: ParadaRequestDto,
  ) {
    return this.field.actualizarParada(this.tenant(u), id, paradaId, dto);
  }
  @Post('rutas/:id/asignar')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Asignar personal/vehículo a la ruta' })
  asignarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: AsignarRutaRequestDto) {
    return this.field.asignarRuta(this.tenant(u), id, dto);
  }

  // ── pedidos ────────────────────────────────────────────────────────────
  @Get('pedidos')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar pedidos de campo' })
  listarPedidos(
    @UsuarioActual() u: UsuarioContexto,
    @Query('estado') estado?: string,
    @Query('rutaId') rutaId?: string,
    @Query('clienteId') clienteId?: string,
  ) {
    return this.field.listarPedidos(this.tenant(u), { estado, rutaId, clienteId });
  }
  @Get('pedidos/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Obtener pedido por id' })
  obtenerPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerPedido(this.tenant(u), id);
  }
  @Post('pedidos')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Crear pedido de campo' })
  crearPedido(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearPedidoRequestDto) {
    return this.field.crearPedido(this.tenant(u), dto);
  }
  @Patch('pedidos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Actualizar pedido de campo' })
  actualizarPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarPedidoRequestDto) {
    return this.field.actualizarPedido(this.tenant(u), id, dto);
  }
  @Delete('pedidos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar pedido de campo' })
  eliminarPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarPedido(this.tenant(u), id);
  }
  @Patch('pedidos/:id/estado')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Cambiar estado del pedido' })
  cambiarEstadoPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: CambiarEstadoPedidoRequestDto) {
    return this.field.cambiarEstadoPedido(this.tenant(u), id, dto);
  }

  // ── asistencia ─────────────────────────────────────────────────────────
  @Get('asistencia')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar asistencia' })
  listarAsistencia(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('fecha') fecha?: string,
  ) {
    return this.field.listarAsistencia(this.tenant(u), { personalId, fecha });
  }
  @Post('asistencia')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Registrar asistencia' })
  registrarAsistencia(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearAsistenciaRequestDto) {
    return this.field.registrarAsistencia(this.tenant(u), dto);
  }

  // ── incidencias ───────────────────────────────────────────────────────
  @Get('incidencias')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar incidencias' })
  listarIncidencias(
    @UsuarioActual() u: UsuarioContexto,
    @Query('estado') estado?: string,
    @Query('rutaId') rutaId?: string,
  ) {
    return this.field.listarIncidencias(this.tenant(u), { estado, rutaId });
  }
  @Get('incidencias/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Obtener incidencia por id' })
  obtenerIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerIncidencia(this.tenant(u), id);
  }
  @Post('incidencias')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Crear incidencia' })
  crearIncidencia(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearIncidenciaRequestDto) {
    return this.field.crearIncidencia(this.tenant(u), dto);
  }
  @Patch('incidencias/:id')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Actualizar incidencia' })
  actualizarIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarIncidenciaRequestDto) {
    return this.field.actualizarIncidencia(this.tenant(u), id, dto);
  }
  @Delete('incidencias/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  @ApiOperation({ summary: 'Eliminar incidencia' })
  eliminarIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarIncidencia(this.tenant(u), id);
  }

  // ── tracking (GPS) ────────────────────────────────────────────────────
  @Get('tracking')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar tracking GPS' })
  listarTracking(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
  ) {
    return this.field.listarTracking(this.tenant(u), { personalId, desde, hasta });
  }
  @Post('tracking')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Registrar tracking GPS (lote)' })
  registrarTracking(@UsuarioActual() u: UsuarioContexto, @Body() dto: TrackingBulkRequestDto) {
    return this.field.registrarTracking(this.tenant(u), dto.registros);
  }

  // ── visitas (telemetria) ──────────────────────────────────────────────
  @Get('visitas')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Listar visitas (telemetría)' })
  listarVisitas(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('fecha') fecha?: string,
  ) {
    return this.field.listarVisitas(this.tenant(u), { personalId, fecha });
  }
  @Post('visitas')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Registrar visita (telemetría)' })
  registrarVisita(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearVisitaRequestDto) {
    return this.field.registrarVisita(this.tenant(u), dto);
  }

  // ── cumplimiento ──────────────────────────────────────────────────────
  @Get('cumplimiento/:rutaId/:fecha')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Obtener cumplimiento de ruta/fecha' })
  obtenerCumplimiento(@UsuarioActual() u: UsuarioContexto, @Param('rutaId') rutaId: string, @Param('fecha') fecha: string) {
    return this.field.obtenerCumplimiento(this.tenant(u), rutaId, fecha);
  }
  @Post('cumplimiento')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  @ApiOperation({ summary: 'Guardar cumplimiento de ruta' })
  guardarCumplimiento(@UsuarioActual() u: UsuarioContexto, @Body() dto: GuardarCumplimientoRequestDto) {
    return this.field.guardarCumplimiento(this.tenant(u), dto);
  }

  // ── sync offline (app-test) ───────────────────────────────────────────
  @Post('sync')
  @Roles(...ROLES_LOGISTICA)
  @ApiOperation({ summary: 'Sincronizar operaciones offline' })
  sincronizar(@UsuarioActual() u: UsuarioContexto, @Body() dto: SyncRequestDto) {
    return this.field.sincronizar(this.tenant(u), dto.operaciones);
  }
}
