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
  @IsString() nombre: string;
  @IsOptional() @IsString() apellido?: string;
  @IsOptional() @IsIn(['conductor', 'auxiliar', 'supervisor', 'coordinador']) cargo?: string;
  @IsOptional() @IsIn(['activo', 'en_ruta', 'descansando', 'inactivo']) estado?: string;
  @IsOptional() @IsString() telefono?: string;
  @IsOptional() @IsEmail() email?: string;
  @IsOptional() @IsString() rutaAsignadaId?: string;
  @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @IsOptional() @IsDateString() horaCheckIn?: string;
  @IsOptional() @IsDateString() horaCheckOut?: string;
  @IsOptional() @IsNumber() ubicacionLat?: number;
  @IsOptional() @IsNumber() ubicacionLng?: number;
  @IsOptional() @IsNumber() ubicacionPrecision?: number;
  @IsOptional() @IsDateString() ubicacionTs?: string;
}
export class ActualizarPersonalRequestDto {
  @IsOptional() @IsString() nombre?: string;
  @IsOptional() @IsString() apellido?: string;
  @IsOptional() @IsIn(['conductor', 'auxiliar', 'supervisor', 'coordinador']) cargo?: string;
  @IsOptional() @IsIn(['activo', 'en_ruta', 'descansando', 'inactivo']) estado?: string;
  @IsOptional() @IsString() telefono?: string;
  @IsOptional() @IsEmail() email?: string;
  @IsOptional() @IsString() rutaAsignadaId?: string;
  @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @IsOptional() @IsDateString() horaCheckIn?: string;
  @IsOptional() @IsDateString() horaCheckOut?: string;
  @IsOptional() @IsNumber() ubicacionLat?: number;
  @IsOptional() @IsNumber() ubicacionLng?: number;
  @IsOptional() @IsNumber() ubicacionPrecision?: number;
  @IsOptional() @IsDateString() ubicacionTs?: string;
}

// ── clientes ─────────────────────────────────────────────────────────────
export class CrearClienteRequestDto {
  @IsString() nombreCompleto: string;
  @IsOptional() @IsString() tipoDocumento?: string;
  @IsOptional() @IsString() numeroDocumento?: string;
  @IsOptional() @IsIn(['particular', 'minorista', 'mayorista', 'corporativo']) tipoCliente?: string;
  @IsOptional() @IsEmail() email?: string;
  @IsOptional() @IsString() telefono?: string;
  @IsOptional() @IsString() telefonoAlternativo?: string;
  @IsOptional() @IsString() direccionPrincipal?: string;
  @IsOptional() @IsString() direccionSecundaria?: string;
  @IsOptional() @IsString() referenciasDireccion?: string;
  @IsOptional() @IsNumber() lat?: number;
  @IsOptional() @IsNumber() lng?: number;
  @IsOptional() @IsString() notasAdicionales?: string;
  @IsOptional() @IsString() urlGoogleMaps?: string;
}
export class ActualizarClienteRequestDto {
  @IsOptional() @IsString() nombreCompleto?: string;
  @IsOptional() @IsString() tipoDocumento?: string;
  @IsOptional() @IsString() numeroDocumento?: string;
  @IsOptional() @IsIn(['particular', 'minorista', 'mayorista', 'corporativo']) tipoCliente?: string;
  @IsOptional() @IsEmail() email?: string;
  @IsOptional() @IsString() telefono?: string;
  @IsOptional() @IsString() telefonoAlternativo?: string;
  @IsOptional() @IsString() direccionPrincipal?: string;
  @IsOptional() @IsString() direccionSecundaria?: string;
  @IsOptional() @IsString() referenciasDireccion?: string;
  @IsOptional() @IsNumber() lat?: number;
  @IsOptional() @IsNumber() lng?: number;
  @IsOptional() @IsString() notasAdicionales?: string;
  @IsOptional() @IsString() urlGoogleMaps?: string;
}

// ── vehiculos ────────────────────────────────────────────────────────────
export class CrearVehiculoRequestDto {
  @IsString() placa: string;
  @IsOptional() @IsIn(['camioneta', 'furgon', 'camion', 'moto', 'otro']) tipo?: string;
  @IsOptional() @IsString() marca?: string;
  @IsOptional() @IsString() modelo?: string;
  @IsOptional() @IsInt() @Min(1900) anio?: number;
  @IsOptional() @IsIn(['disponible', 'en_ruta', 'mantenimiento', 'fuera_de_servicio']) estado?: string;
  @IsOptional() @IsString() color?: string;
  @IsOptional() @IsNumber() capacidadCargaKg?: number;
  @IsOptional() @IsString() numeroChasis?: string;
  @IsOptional() @IsString() numeroMotor?: string;
  @IsOptional() @IsString() tipoCombustible?: string;
  @IsOptional() @IsDateString() vencimientoSeguro?: string;
  @IsOptional() @IsDateString() vencimientoCirculacion?: string;
  @IsOptional() @IsString() notasAdicionales?: string;
  @IsOptional() @IsString() conductorId?: string;
  @IsOptional() @IsString() rutaActivaId?: string;
}
export class ActualizarVehiculoRequestDto {
  @IsOptional() @IsString() placa?: string;
  @IsOptional() @IsIn(['camioneta', 'furgon', 'camion', 'moto', 'otro']) tipo?: string;
  @IsOptional() @IsString() marca?: string;
  @IsOptional() @IsString() modelo?: string;
  @IsOptional() @IsInt() @Min(1900) anio?: number;
  @IsOptional() @IsIn(['disponible', 'en_ruta', 'mantenimiento', 'fuera_de_servicio']) estado?: string;
  @IsOptional() @IsString() color?: string;
  @IsOptional() @IsNumber() capacidadCargaKg?: number;
  @IsOptional() @IsString() numeroChasis?: string;
  @IsOptional() @IsString() numeroMotor?: string;
  @IsOptional() @IsString() tipoCombustible?: string;
  @IsOptional() @IsDateString() vencimientoSeguro?: string;
  @IsOptional() @IsDateString() vencimientoCirculacion?: string;
  @IsOptional() @IsString() notasAdicionales?: string;
  @IsOptional() @IsString() conductorId?: string;
  @IsOptional() @IsString() rutaActivaId?: string;
}

// ── rutas ────────────────────────────────────────────────────────────────
export class CrearRutaRequestDto {
  @IsString() nombre: string;
  @IsOptional() @IsString() descripcion?: string;
  @IsOptional() @IsIn(['pendiente', 'en_curso', 'completada', 'cancelada']) estado?: string;
  @IsOptional() @IsArray() personalIds?: string[];
  @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @IsOptional() @IsDateString() fechaInicio?: string;
  @IsOptional() @IsDateString() fechaFin?: string;
}
export class ActualizarRutaRequestDto {
  @IsOptional() @IsString() nombre?: string;
  @IsOptional() @IsString() descripcion?: string;
  @IsOptional() @IsIn(['pendiente', 'en_curso', 'completada', 'cancelada']) estado?: string;
  @IsOptional() @IsArray() personalIds?: string[];
  @IsOptional() @IsString() vehiculoAsignadoId?: string;
  @IsOptional() @IsDateString() fechaInicio?: string;
  @IsOptional() @IsDateString() fechaFin?: string;
}
export class ParadaRequestDto {
  @IsOptional() @IsString() rutaId?: string;
  @IsOptional() @IsInt() @Min(0) orden?: number;
  @IsString() nombre: string;
  @IsOptional() @IsString() direccion?: string;
  @IsOptional() @IsNumber() lat?: number;
  @IsOptional() @IsNumber() lng?: number;
  @IsOptional() @IsBoolean() completada?: boolean;
  @IsOptional() @IsString() pedidoId?: string;
  @IsOptional() @IsIn(['cliente', 'logistica']) tipo?: string;
  @IsOptional() @IsString() clienteId?: string;
  @IsOptional() @IsString() telefono?: string;
  @IsOptional() @IsString() referencias?: string;
  @IsOptional() @IsString() horarioAtencion?: string;
  @IsOptional() @IsString() notas?: string;
  @IsOptional() @IsString() tipoCombustible?: string;
}
export class AsignarRutaRequestDto {
  @IsOptional() @IsArray() personalIds?: string[];
  @IsOptional() @IsString() vehiculoId?: string;
}

// ── pedidos ──────────────────────────────────────────────────────────────
export class CrearPedidoRequestDto {
  @IsOptional() @IsString() cliente?: string;
  @IsOptional() @IsString() clienteId?: string;
  @IsOptional() @IsString() direccionEntrega?: string;
  @IsOptional() @IsNumber() lat?: number;
  @IsOptional() @IsNumber() lng?: number;
  @IsOptional() @IsIn(['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado']) estado?: string;
  @IsOptional() @IsString() rutaId?: string;
  @IsOptional() @IsString() notas?: string;
}
export class ActualizarPedidoRequestDto {
  @IsOptional() @IsString() cliente?: string;
  @IsOptional() @IsString() clienteId?: string;
  @IsOptional() @IsString() direccionEntrega?: string;
  @IsOptional() @IsNumber() lat?: number;
  @IsOptional() @IsNumber() lng?: number;
  @IsOptional() @IsIn(['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado']) estado?: string;
  @IsOptional() @IsString() rutaId?: string;
  @IsOptional() @IsString() notas?: string;
}
export class CambiarEstadoPedidoRequestDto {
  @IsIn(['pendiente', 'asignado', 'en_camino', 'entregado', 'fallido', 'cancelado']) estado: string;
  @IsOptional() @IsString() motivo?: string;
}

// ── asistencia ────────────────────────────────────────────────────────────
export class CrearAsistenciaRequestDto {
  @IsString() personalId: string;
  @IsIn(['entrada', 'salida']) tipo: string;
  @IsOptional() @IsDateString() timestamp?: string;
  @IsOptional() @IsNumber() lat?: number;
  @IsOptional() @IsNumber() lng?: number;
  @IsIn(['puntual', 'tardanza', 'salida_anticipada', 'salida_tardia']) estadoPuntualidad: string;
  @IsOptional() @IsBoolean() enSede?: boolean;
  @IsOptional() @IsString() horaRealLlegada?: string;
  @IsOptional() @IsInt() minutosRetrasoReal?: number;
  @IsOptional() @IsBoolean() monitoreoActivo?: boolean;
  @IsOptional() @IsString() notas?: string;
  @IsOptional() @IsString() justificacion?: string;
  @IsOptional() @IsArray() fotosJustificacion?: string[];
}

// ── incidencias ───────────────────────────────────────────────────────────
export class CrearIncidenciaRequestDto {
  @IsIn(['mecanica', 'accidente', 'salud', 'clima', 'otro']) tipo: string;
  @IsOptional() @IsIn(['abierta', 'en_progreso', 'resuelta']) estado?: string;
  @IsString() descripcion: string;
  @IsOptional() @IsString() personalId?: string;
  @IsOptional() @IsString() vehiculoId?: string;
  @IsOptional() @IsString() rutaId?: string;
  @IsOptional() @IsString() resolucion?: string;
}
export class ActualizarIncidenciaRequestDto {
  @IsOptional() @IsIn(['mecanica', 'accidente', 'salud', 'clima', 'otro']) tipo?: string;
  @IsOptional() @IsIn(['abierta', 'en_progreso', 'resuelta']) estado?: string;
  @IsOptional() @IsString() descripcion?: string;
  @IsOptional() @IsString() personalId?: string;
  @IsOptional() @IsString() vehiculoId?: string;
  @IsOptional() @IsString() rutaId?: string;
  @IsOptional() @IsString() resolucion?: string;
}

// ── tracking ──────────────────────────────────────────────────────────────
export class CrearTrackingRequestDto {
  @IsString() personalId: string;
  @IsNumber() latitud: number;
  @IsNumber() longitud: number;
  @IsOptional() @IsNumber() precision?: number;
  @IsOptional() @IsNumber() velocidad?: number;
  @IsOptional() @IsNumber() rumbo?: number;
  @IsOptional() @IsDateString() timestamp?: string;
}
export class TrackingBulkRequestDto {
  @IsArray() registros: CrearTrackingRequestDto[];
}

// ── visitas (telemetria) ─────────────────────────────────────────────────
export class CrearVisitaRequestDto {
  @IsString() personalId: string;
  @IsOptional() @IsString() clienteId?: string;
  @IsOptional() @IsString() rutaId?: string;
  @IsIn(['impulsacion', 'venta', 'entrega', 'reparto']) tipoActividad: string;
  @IsOptional() @IsIn(['visitado', 'cerca_no_visitado', 'no_visitado']) resultado?: string;
  @IsOptional() @IsNumber() lat?: number;
  @IsOptional() @IsNumber() lng?: number;
  @IsOptional() @IsNumber() distanciaAlCliente?: number;
  @IsOptional() @IsString() horaLlegada?: string;
  @IsOptional() @IsString() horaSalida?: string;
  @IsOptional() @IsInt() duracionMinutos?: number;
  @IsOptional() @IsString() notas?: string;
}

// ── cumplimiento ──────────────────────────────────────────────────────────
export class GuardarCumplimientoRequestDto {
  @IsString() rutaId: string;
  @IsDateString() fecha: string;
  @IsObject() metricas: Record<string, unknown>;
}

// ── ubicacion de personal ─────────────────────────────────────────────────
export class UbicacionRequestDto {
  @IsNumber() lat: number;
  @IsNumber() lng: number;
  @IsOptional() @IsNumber() precision?: number;
  @IsOptional() @IsDateString() timestamp?: string;
}

// ── sync offline ─────────────────────────────────────────────────────────
export class SyncOperacionRequestDto {
  @IsString() tipo: string;
  @IsObject() payload: Record<string, unknown>;
}
export class SyncRequestDto {
  @IsArray() operaciones: SyncOperacionRequestDto[];
}

