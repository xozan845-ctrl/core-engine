import { Body, Controller, Delete, Get, Param, Patch, Post, Query } from '@nestjs/common';
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
  CrearPersonalDto,
  ActualizarPersonalDto,
  CrearClienteDto,
  ActualizarClienteDto,
  CrearVehiculoDto,
  ActualizarVehiculoDto,
  CrearRutaDto,
  ActualizarRutaDto,
  ParadaDto,
  AsignarRutaDto,
  CrearPedidoDto,
  ActualizarPedidoDto,
  CambiarEstadoPedidoDto,
  CrearAsistenciaDto,
  CrearIncidenciaDto,
  ActualizarIncidenciaDto,
  CrearTrackingDto,
  CrearVisitaDto,
  GuardarCumplimientoDto,
  UbicacionDto,
  SyncDto,
  TrackingBulkDto,
} from './field.dtos';

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
  listarPersonal(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarPersonal(this.tenant(u));
  }
  @Get('personal/:id')
  @Roles(...ROLES_LOGISTICA)
  obtenerPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerPersonal(this.tenant(u), id);
  }
  @Post('personal')
  @Roles(...ROLES_LOGISTICA)
  crearPersonal(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearPersonalDto) {
    return this.field.crearPersonal(this.tenant(u), dto);
  }
  @Patch('personal/:id')
  @Roles(...ROLES_LOGISTICA)
  actualizarPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarPersonalDto) {
    return this.field.actualizarPersonal(this.tenant(u), id, dto);
  }
  @Delete('personal/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  eliminarPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarPersonal(this.tenant(u), id);
  }
  @Get('personal/:id/ubicacion')
  @Roles(...ROLES_LOGISTICA)
  ubicacionPersonal(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.ubicacionPersonal(this.tenant(u), id);
  }
  @Patch('personal/:id/ubicacion')
  @Roles(...ROLES_LOGISTICA)
  actualizarUbicacion(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: UbicacionDto) {
    return this.field.actualizarUbicacion(this.tenant(u), id, dto);
  }

  // ── clientes ───────────────────────────────────────────────────────────
  @Get('clientes')
  @Roles(...ROLES_LOGISTICA)
  listarClientes(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarClientes(this.tenant(u));
  }
  @Get('clientes/:id')
  @Roles(...ROLES_LOGISTICA)
  obtenerCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerCliente(this.tenant(u), id);
  }
  @Post('clientes')
  @Roles(...ROLES_LOGISTICA)
  crearCliente(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearClienteDto) {
    return this.field.crearCliente(this.tenant(u), dto);
  }
  @Patch('clientes/:id')
  @Roles(...ROLES_LOGISTICA)
  actualizarCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarClienteDto) {
    return this.field.actualizarCliente(this.tenant(u), id, dto);
  }
  @Delete('clientes/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  eliminarCliente(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarCliente(this.tenant(u), id);
  }

  // ── vehiculos ──────────────────────────────────────────────────────────
  @Get('vehiculos')
  @Roles(...ROLES_LOGISTICA)
  listarVehiculos(@UsuarioActual() u: UsuarioContexto) {
    return this.field.listarVehiculos(this.tenant(u));
  }
  @Get('vehiculos/:id')
  @Roles(...ROLES_LOGISTICA)
  obtenerVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerVehiculo(this.tenant(u), id);
  }
  @Post('vehiculos')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  crearVehiculo(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearVehiculoDto) {
    return this.field.crearVehiculo(this.tenant(u), dto);
  }
  @Patch('vehiculos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  actualizarVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarVehiculoDto) {
    return this.field.actualizarVehiculo(this.tenant(u), id, dto);
  }
  @Delete('vehiculos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  eliminarVehiculo(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarVehiculo(this.tenant(u), id);
  }

  // ── rutas ──────────────────────────────────────────────────────────────
  @Get('rutas')
  @Roles(...ROLES_LOGISTICA)
  listarRutas(@UsuarioActual() u: UsuarioContexto, @Query('personalId') personalId?: string) {
    return this.field.listarRutas(this.tenant(u), personalId);
  }
  @Get('rutas/:id')
  @Roles(...ROLES_LOGISTICA)
  obtenerRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerRuta(this.tenant(u), id);
  }
  @Post('rutas')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  crearRuta(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearRutaDto) {
    return this.field.crearRuta(this.tenant(u), dto);
  }
  @Patch('rutas/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  actualizarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarRutaDto) {
    return this.field.actualizarRuta(this.tenant(u), id, dto);
  }
  @Delete('rutas/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  eliminarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarRuta(this.tenant(u), id);
  }
  @Post('rutas/:id/paradas')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  agregarParada(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ParadaDto) {
    return this.field.agregarParada(this.tenant(u), id, dto);
  }
  @Patch('rutas/:id/paradas/:paradaId')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  actualizarParada(
    @UsuarioActual() u: UsuarioContexto,
    @Param('id') id: string,
    @Param('paradaId') paradaId: string,
    @Body() dto: ParadaDto,
  ) {
    return this.field.actualizarParada(this.tenant(u), id, paradaId, dto);
  }
  @Post('rutas/:id/asignar')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  asignarRuta(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: AsignarRutaDto) {
    return this.field.asignarRuta(this.tenant(u), id, dto);
  }

  // ── pedidos ────────────────────────────────────────────────────────────
  @Get('pedidos')
  @Roles(...ROLES_LOGISTICA)
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
  obtenerPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerPedido(this.tenant(u), id);
  }
  @Post('pedidos')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  crearPedido(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearPedidoDto) {
    return this.field.crearPedido(this.tenant(u), dto);
  }
  @Patch('pedidos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  actualizarPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarPedidoDto) {
    return this.field.actualizarPedido(this.tenant(u), id, dto);
  }
  @Delete('pedidos/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  eliminarPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarPedido(this.tenant(u), id);
  }
  @Patch('pedidos/:id/estado')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  cambiarEstadoPedido(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: CambiarEstadoPedidoDto) {
    return this.field.cambiarEstadoPedido(this.tenant(u), id, dto);
  }

  // ── asistencia ─────────────────────────────────────────────────────────
  @Get('asistencia')
  @Roles(...ROLES_LOGISTICA)
  listarAsistencia(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('fecha') fecha?: string,
  ) {
    return this.field.listarAsistencia(this.tenant(u), { personalId, fecha });
  }
  @Post('asistencia')
  @Roles(...ROLES_LOGISTICA)
  registrarAsistencia(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearAsistenciaDto) {
    return this.field.registrarAsistencia(this.tenant(u), dto);
  }

  // ── incidencias ───────────────────────────────────────────────────────
  @Get('incidencias')
  @Roles(...ROLES_LOGISTICA)
  listarIncidencias(
    @UsuarioActual() u: UsuarioContexto,
    @Query('estado') estado?: string,
    @Query('rutaId') rutaId?: string,
  ) {
    return this.field.listarIncidencias(this.tenant(u), { estado, rutaId });
  }
  @Get('incidencias/:id')
  @Roles(...ROLES_LOGISTICA)
  obtenerIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.obtenerIncidencia(this.tenant(u), id);
  }
  @Post('incidencias')
  @Roles(...ROLES_LOGISTICA)
  crearIncidencia(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearIncidenciaDto) {
    return this.field.crearIncidencia(this.tenant(u), dto);
  }
  @Patch('incidencias/:id')
  @Roles(...ROLES_LOGISTICA)
  actualizarIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string, @Body() dto: ActualizarIncidenciaDto) {
    return this.field.actualizarIncidencia(this.tenant(u), id, dto);
  }
  @Delete('incidencias/:id')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR)
  eliminarIncidencia(@UsuarioActual() u: UsuarioContexto, @Param('id') id: string) {
    return this.field.eliminarIncidencia(this.tenant(u), id);
  }

  // ── tracking (GPS) ────────────────────────────────────────────────────
  @Get('tracking')
  @Roles(...ROLES_LOGISTICA)
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
  registrarTracking(@UsuarioActual() u: UsuarioContexto, @Body() dto: TrackingBulkDto) {
    return this.field.registrarTracking(this.tenant(u), dto.registros);
  }

  // ── visitas (telemetria) ──────────────────────────────────────────────
  @Get('visitas')
  @Roles(...ROLES_LOGISTICA)
  listarVisitas(
    @UsuarioActual() u: UsuarioContexto,
    @Query('personalId') personalId?: string,
    @Query('fecha') fecha?: string,
  ) {
    return this.field.listarVisitas(this.tenant(u), { personalId, fecha });
  }
  @Post('visitas')
  @Roles(...ROLES_LOGISTICA)
  registrarVisita(@UsuarioActual() u: UsuarioContexto, @Body() dto: CrearVisitaDto) {
    return this.field.registrarVisita(this.tenant(u), dto);
  }

  // ── cumplimiento ──────────────────────────────────────────────────────
  @Get('cumplimiento/:rutaId/:fecha')
  @Roles(...ROLES_LOGISTICA)
  obtenerCumplimiento(@UsuarioActual() u: UsuarioContexto, @Param('rutaId') rutaId: string, @Param('fecha') fecha: string) {
    return this.field.obtenerCumplimiento(this.tenant(u), rutaId, fecha);
  }
  @Post('cumplimiento')
  @Roles(ROLES.ADMIN, ROLES.COORDINADOR, ROLES.SUPERVISOR)
  guardarCumplimiento(@UsuarioActual() u: UsuarioContexto, @Body() dto: GuardarCumplimientoDto) {
    return this.field.guardarCumplimiento(this.tenant(u), dto);
  }

  // ── sync offline (app-test) ───────────────────────────────────────────
  @Post('sync')
  @Roles(...ROLES_LOGISTICA)
  sincronizar(@UsuarioActual() u: UsuarioContexto, @Body() dto: SyncDto) {
    return this.field.sincronizar(this.tenant(u), dto.operaciones);
  }
}
