import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiTags,
} from '@nestjs/swagger';
import {
  CAPACIDADES,
  DomainError,
  RequiereCapacidades,
  Permisos,
  ROLES,
  ROLES_LOGISTICA,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import {
  CrearBloqueoRequestDto,
  CrearCitaRequestDto,
  CrearDisponibilidadRequestDto,
  CrearRecursoRequestDto,
} from './scheduling.dtos';
import {
  BloqueoResponseDto,
  CitaResponseDto,
  DisponibilidadResponseDto,
  RecordatorioResponseDto,
  RecursoResponseDto,
} from './scheduling.response.dto';
import { SchedulingService } from './scheduling.service';

const ROLES_AGENDA = [ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO, ...ROLES_LOGISTICA];

@ApiTags('Scheduling · Agenda')
@ApiBearerAuth()
@Controller('api/v2/scheduling')
@Roles(...ROLES_AGENDA)
@RequiereCapacidades(CAPACIDADES.AGENDA_RECURSOS)
export class SchedulingController {
  constructor(private readonly scheduling: SchedulingService) {}

  @Get('recursos')
  @Permisos('agenda.leer')
  @ApiOperation({ summary: 'Listar recursos reservables de la empresa activa' })
  @ApiOkResponse({ type: [RecursoResponseDto] })
  recursos(@UsuarioActual() usuario: UsuarioContexto) {
    return this.scheduling.listarRecursos(this.tenant(usuario));
  }

  @Post('recursos')
  @Permisos('agenda.escribir')
  @ApiOperation({ summary: 'Crear un recurso reservable' })
  @ApiCreatedResponse({ type: RecursoResponseDto })
  crearRecurso(@UsuarioActual() usuario: UsuarioContexto, @Body() dto: CrearRecursoRequestDto) {
    return this.scheduling.crearRecurso(this.tenant(usuario), dto);
  }

  @Get('recursos/:recursoId/disponibilidad')
  @Permisos('agenda.leer')
  @ApiOperation({ summary: 'Listar disponibilidad semanal de un recurso' })
  @ApiParam({ name: 'recursoId', format: 'uuid' })
  @ApiOkResponse({ type: [DisponibilidadResponseDto] })
  disponibilidad(@UsuarioActual() usuario: UsuarioContexto, @Param('recursoId') recursoId: string) {
    return this.scheduling.listarDisponibilidad(this.tenant(usuario), recursoId);
  }

  @Post('disponibilidad')
  @Permisos('agenda.escribir')
  @ApiOperation({ summary: 'Configurar un intervalo semanal de disponibilidad' })
  @ApiCreatedResponse({ type: DisponibilidadResponseDto })
  crearDisponibilidad(@UsuarioActual() usuario: UsuarioContexto, @Body() dto: CrearDisponibilidadRequestDto) {
    return this.scheduling.crearDisponibilidad(this.tenant(usuario), dto);
  }

  @Post('bloqueos')
  @Permisos('agenda.escribir')
  @ApiOperation({ summary: 'Bloquear un recurso en un intervalo' })
  @ApiCreatedResponse({ type: BloqueoResponseDto })
  crearBloqueo(@UsuarioActual() usuario: UsuarioContexto, @Body() dto: CrearBloqueoRequestDto) {
    return this.scheduling.crearBloqueo(this.tenant(usuario), usuario.user_id, dto);
  }

  @Get('citas')
  @Permisos('agenda.leer')
  @ApiOperation({ summary: 'Listar citas de la empresa activa' })
  @ApiOkResponse({ type: [CitaResponseDto] })
  citas(@UsuarioActual() usuario: UsuarioContexto, @Query('desde') desde?: string, @Query('hasta') hasta?: string) {
    return this.scheduling.listarCitas(this.tenant(usuario), desde, hasta);
  }

  @Post('citas')
  @Permisos('agenda.escribir')
  @ApiOperation({ summary: 'Crear una cita con validacion de disponibilidad' })
  @ApiCreatedResponse({ type: CitaResponseDto })
  crearCita(@UsuarioActual() usuario: UsuarioContexto, @Body() dto: CrearCitaRequestDto) {
    return this.scheduling.crearCita(this.tenant(usuario), dto);
  }

  @Patch('citas/:citaId/cancelar')
  @Permisos('agenda.escribir')
  @ApiOperation({ summary: 'Cancelar una cita activa' })
  @ApiParam({ name: 'citaId', format: 'uuid' })
  @ApiOkResponse({ type: CitaResponseDto })
  cancelar(@UsuarioActual() usuario: UsuarioContexto, @Param('citaId') citaId: string, @Body('motivo') motivo?: string) {
    return this.scheduling.cancelarCita(this.tenant(usuario), citaId, motivo);
  }

  @Get('recordatorios')
  @Permisos('agenda.leer')
  @ApiOperation({ summary: 'Listar recordatorios de citas' })
  @ApiOkResponse({ type: [RecordatorioResponseDto] })
  recordatorios(@UsuarioActual() usuario: UsuarioContexto) {
    return this.scheduling.listarRecordatorios(this.tenant(usuario));
  }

  private tenant(usuario?: UsuarioContexto): string {
    if (!usuario?.tenant_id) throw new DomainError('TENANT_REQUERIDO', 'Se requiere una empresa activa.');
    return usuario.tenant_id;
  }
}
