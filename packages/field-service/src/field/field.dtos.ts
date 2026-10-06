import { ApiProperty } from '@nestjs/swagger';
import {
  IsArray,
  IsBoolean,
  IsDateString,
  IsEmail,
  IsIn,
  IsInt,
  IsNumber,
  IsObject,
  IsOptional,
  IsString,
  Min,
} from 'class-validator';

// ── personal ─────────────────────────────────────────────────────────────
export class CrearPersonalRequestDto {
  @ApiProperty() @IsString() nombre: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() apellido?: string;
  @ApiProperty({ required: false, enum: ['conductor', 'auxiliar', 'supervisor', 'coordinador'] }) @IsOptional() @IsIn(['conductor', 'auxiliar', 'supervisor', 'coordinador']) cargo?: string;
  @ApiProperty({ required: false, enum: ['activo', 'en_ruta', 'descansando', 'inactivo'] }) @IsOptional() @IsIn(['activo', 'en_ruta', 'descansando', 'inactivo']) estado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefono?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsEmail() email?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaAsignadaId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() horaCheckIn?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() horaCheckOut?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() ubicacionLat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() ubicacionLng?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() ubicacionPrecision?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() ubicacionTs?: string;
}
export class ActualizarPersonalRequestDto {
  @ApiProperty({ required: false }) @IsOptional() @IsString() nombre?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() apellido?: string;
  @ApiProperty({ required: false, enum: ['conductor', 'auxiliar', 'supervisor', 'coordinador'] }) @IsOptional() @IsIn(['conductor', 'auxiliar', 'supervisor', 'coordinador']) cargo?: string;
  @ApiProperty({ required: false, enum: ['activo', 'en_ruta', 'descansando', 'inactivo'] }) @IsOptional() @IsIn(['activo', 'en_ruta', 'descansando', 'inactivo']) estado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefono?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsEmail() email?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaAsignadaId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() horaCheckIn?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() horaCheckOut?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() ubicacionLat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() ubicacionLng?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() ubicacionPrecision?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() ubicacionTs?: string;
}

// ── clientes ─────────────────────────────────────────────────────────────
export class CrearClienteRequestDto {
  @ApiProperty() @IsString() nombreCompleto: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() tipoDocumento?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() numeroDocumento?: string;
  @ApiProperty({ required: false, enum: ['particular', 'minorista', 'mayorista', 'corporativo'] }) @IsOptional() @IsIn(['particular', 'minorista', 'mayorista', 'corporativo']) tipoCliente?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsEmail() email?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefono?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefonoAlternativo?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() direccionPrincipal?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() direccionSecundaria?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() referenciasDireccion?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lng?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notasAdicionales?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() urlGoogleMaps?: string;
}
export class ActualizarClienteRequestDto {
  @ApiProperty({ required: false }) @IsOptional() @IsString() nombreCompleto?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() tipoDocumento?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() numeroDocumento?: string;
  @ApiProperty({ required: false, enum: ['particular', 'minorista', 'mayorista', 'corporativo'] }) @IsOptional() @IsIn(['particular', 'minorista', 'mayorista', 'corporativo']) tipoCliente?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsEmail() email?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefono?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefonoAlternativo?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() direccionPrincipal?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() direccionSecundaria?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() referenciasDireccion?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lng?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notasAdicionales?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() urlGoogleMaps?: string;
}

// ── vehiculos ────────────────────────────────────────────────────────────
export class CrearVehiculoRequestDto {
  @ApiProperty() @IsString() placa: string;
  @ApiProperty({ required: false, enum: ['camioneta', 'furgon', 'camion', 'moto', 'otro'] }) @IsOptional() @IsIn(['camioneta', 'furgon', 'camion', 'moto', 'otro']) tipo?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() marca?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() modelo?: string;
  @ApiProperty({ required: false, minimum: 1900 }) @IsOptional() @IsInt() @Min(1900) anio?: number;
  @ApiProperty({ required: false, enum: ['disponible', 'en_ruta', 'mantenimiento', 'fuera_de_servicio'] }) @IsOptional() @IsIn(['disponible', 'en_ruta', 'mantenimiento', 'fuera_de_servicio']) estado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() color?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() capacidadCargaKg?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsString() numeroChasis?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() numeroMotor?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() tipoCombustible?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() vencimientoSeguro?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() vencimientoCirculacion?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notasAdicionales?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() conductorId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaActivaId?: string;
}
export class ActualizarVehiculoRequestDto {
  @ApiProperty({ required: false }) @IsOptional() @IsString() placa?: string;
  @ApiProperty({ required: false, enum: ['camioneta', 'furgon', 'camion', 'moto', 'otro'] }) @IsOptional() @IsIn(['camioneta', 'furgon', 'camion', 'moto', 'otro']) tipo?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() marca?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() modelo?: string;
  @ApiProperty({ required: false, minimum: 1900 }) @IsOptional() @IsInt() @Min(1900) anio?: number;
  @ApiProperty({ required: false, enum: ['disponible', 'en_ruta', 'mantenimiento', 'fuera_de_servicio'] }) @IsOptional() @IsIn(['disponible', 'en_ruta', 'mantenimiento', 'fuera_de_servicio']) estado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() color?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() capacidadCargaKg?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsString() numeroChasis?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() numeroMotor?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() tipoCombustible?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() vencimientoSeguro?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() vencimientoCirculacion?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notasAdicionales?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() conductorId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaActivaId?: string;
}

// ── rutas ────────────────────────────────────────────────────────────────
export class CrearRutaRequestDto {
  @ApiProperty() @IsString() nombre: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() descripcion?: string;
  @ApiProperty({ required: false, enum: ['pendiente', 'en_curso', 'completada', 'cancelada'] }) @IsOptional() @IsIn(['pendiente', 'en_curso', 'completada', 'cancelada']) estado?: string;
  @ApiProperty({ required: false, type: [String] }) @IsOptional() @IsArray() personalIds?: string[];
  @ApiProperty({ required: false }) @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() fechaInicio?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() fechaFin?: string;
}
export class ActualizarRutaRequestDto {
  @ApiProperty({ required: false }) @IsOptional() @IsString() nombre?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() descripcion?: string;
  @ApiProperty({ required: false, enum: ['pendiente', 'en_curso', 'completada', 'cancelada'] }) @IsOptional() @IsIn(['pendiente', 'en_curso', 'completada', 'cancelada']) estado?: string;
  @ApiProperty({ required: false, type: [String] }) @IsOptional() @IsArray() personalIds?: string[];
  @ApiProperty({ required: false }) @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() fechaInicio?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() fechaFin?: string;
}
export class ParadaRequestDto {
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaId?: string;
  @ApiProperty({ required: false, minimum: 0 }) @IsOptional() @IsInt() @Min(0) orden?: number;
  @ApiProperty() @IsString() nombre: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() direccion?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lng?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsBoolean() completada?: boolean;
  @ApiProperty({ required: false }) @IsOptional() @IsString() pedidoId?: string;
  @ApiProperty({ required: false, enum: ['cliente', 'logistica'] }) @IsOptional() @IsIn(['cliente', 'logistica']) tipo?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() clienteId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefono?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() referencias?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() horarioAtencion?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notas?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() tipoCombustible?: string;
}
export class AsignarRutaRequestDto {
  @ApiProperty({ required: false, type: [String] }) @IsOptional() @IsArray() personalIds?: string[];
  @ApiProperty({ required: false }) @IsOptional() @IsString() vehiculoId?: string;
}

// ── pedidos ──────────────────────────────────────────────────────────────
export class CrearPedidoRequestDto {
  @ApiProperty({ required: false }) @IsOptional() @IsString() cliente?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() clienteId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() direccionEntrega?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lng?: number;
  @ApiProperty({ required: false, enum: ['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado'] }) @IsOptional() @IsIn(['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado']) estado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notas?: string;
}
export class ActualizarPedidoRequestDto {
  @ApiProperty({ required: false }) @IsOptional() @IsString() cliente?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() clienteId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() direccionEntrega?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lng?: number;
  @ApiProperty({ required: false, enum: ['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado'] }) @IsOptional() @IsIn(['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado']) estado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notas?: string;
}
export class CambiarEstadoPedidoRequestDto {
  @ApiProperty({ enum: ['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado'] }) @IsIn(['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado']) estado: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() motivo?: string;
}

// ── asistencia ────────────────────────────────────────────────────────────
export class CrearAsistenciaRequestDto {
  @ApiProperty() @IsString() personalId: string;
  @ApiProperty({ enum: ['entrada', 'salida'] }) @IsIn(['entrada', 'salida']) tipo: string;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() timestamp?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lng?: number;
  @ApiProperty({ enum: ['puntual', 'tardanza', 'salida_anticipada', 'salida_tardia'] }) @IsIn(['puntual', 'tardanza', 'salida_anticipada', 'salida_tardia']) estadoPuntualidad: string;
  @ApiProperty({ required: false }) @IsOptional() @IsBoolean() enSede?: boolean;
  @ApiProperty({ required: false }) @IsOptional() @IsString() horaRealLlegada?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsInt() minutosRetrasoReal?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsBoolean() monitoreoActivo?: boolean;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notas?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() justificacion?: string;
  @ApiProperty({ required: false, type: [String] }) @IsOptional() @IsArray() fotosJustificacion?: string[];
}

// ── incidencias ───────────────────────────────────────────────────────────
export class CrearIncidenciaRequestDto {
  @ApiProperty({ enum: ['mecanica', 'accidente', 'salud', 'clima', 'otro'] }) @IsIn(['mecanica', 'accidente', 'salud', 'clima', 'otro']) tipo: string;
  @ApiProperty({ required: false, enum: ['abierta', 'en_progreso', 'resuelta'] }) @IsOptional() @IsIn(['abierta', 'en_progreso', 'resuelta']) estado?: string;
  @ApiProperty() @IsString() descripcion: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() personalId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() vehiculoId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() resolucion?: string;
}
export class ActualizarIncidenciaRequestDto {
  @ApiProperty({ required: false, enum: ['mecanica', 'accidente', 'salud', 'clima', 'otro'] }) @IsOptional() @IsIn(['mecanica', 'accidente', 'salud', 'clima', 'otro']) tipo?: string;
  @ApiProperty({ required: false, enum: ['abierta', 'en_progreso', 'resuelta'] }) @IsOptional() @IsIn(['abierta', 'en_progreso', 'resuelta']) estado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() descripcion?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() personalId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() vehiculoId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() resolucion?: string;
}

// ── tracking ──────────────────────────────────────────────────────────────
export class CrearTrackingRequestDto {
  @ApiProperty() @IsString() personalId: string;
  @ApiProperty() @IsNumber() latitud: number;
  @ApiProperty() @IsNumber() longitud: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() precision?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() velocidad?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() rumbo?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() timestamp?: string;
}
export class TrackingBulkRequestDto {
  @ApiProperty({ type: [CrearTrackingRequestDto] }) @IsArray() registros: CrearTrackingRequestDto[];
}

// ── visitas (telemetria) ─────────────────────────────────────────────────
export class CrearVisitaRequestDto {
  @ApiProperty() @IsString() personalId: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() clienteId?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() rutaId?: string;
  @ApiProperty({ enum: ['impulsacion', 'venta', 'entrega', 'reparto'] }) @IsIn(['impulsacion', 'venta', 'entrega', 'reparto']) tipoActividad: string;
  @ApiProperty({ required: false, enum: ['visitado', 'cerca_no_visitado', 'no_visitado'] }) @IsOptional() @IsIn(['visitado', 'cerca_no_visitado', 'no_visitado']) resultado?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lat?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() lng?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() distanciaAlCliente?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsString() horaLlegada?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() horaSalida?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsInt() duracionMinutos?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notas?: string;
}

// ── cumplimiento ──────────────────────────────────────────────────────────
export class GuardarCumplimientoRequestDto {
  @ApiProperty() @IsString() rutaId: string;
  @ApiProperty() @IsDateString() fecha: string;
  @ApiProperty({ type: Object }) @IsObject() metricas: Record<string, unknown>;
}

// ── ubicacion de personal ─────────────────────────────────────────────────
export class UbicacionRequestDto {
  @ApiProperty() @IsNumber() lat: number;
  @ApiProperty() @IsNumber() lng: number;
  @ApiProperty({ required: false }) @IsOptional() @IsNumber() precision?: number;
  @ApiProperty({ required: false }) @IsOptional() @IsDateString() timestamp?: string;
}

// ── sync offline ─────────────────────────────────────────────────────────
export class SyncOperacionRequestDto {
  @ApiProperty() @IsString() tipo: string;
  @ApiProperty({ type: Object }) @IsObject() payload: Record<string, unknown>;
}
export class SyncRequestDto {
  @ApiProperty({ type: [SyncOperacionRequestDto] }) @IsArray() operaciones: SyncOperacionRequestDto[];
}
