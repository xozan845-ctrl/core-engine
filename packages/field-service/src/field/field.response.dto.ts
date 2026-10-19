import { ApiProperty } from '@nestjs/swagger';

export class PersonalResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty() nombre: string;
  @ApiProperty() apellido: string;
  @ApiProperty() cargo: string;
  @ApiProperty() estado: string;
  @ApiProperty({ required: false, nullable: true }) telefono: string | null;
  @ApiProperty({ required: false, nullable: true }) email: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) ruta_asignada_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) vehiculo_asignado_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) hora_check_in: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) hora_check_out: string | null;
  @ApiProperty({ required: false, nullable: true }) ubicacion_lat: number | null;
  @ApiProperty({ required: false, nullable: true }) ubicacion_lng: number | null;
  @ApiProperty({ required: false, nullable: true }) ubicacion_precision: number | null;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) ubicacion_ts: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class ClienteResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty() nombre_completo: string;
  @ApiProperty({ required: false, nullable: true }) tipo_documento: string | null;
  @ApiProperty({ required: false, nullable: true }) numero_documento: string | null;
  @ApiProperty() tipo_cliente: string;
  @ApiProperty({ required: false, nullable: true }) email: string | null;
  @ApiProperty({ required: false, nullable: true }) telefono: string | null;
  @ApiProperty({ required: false, nullable: true }) telefono_alternativo: string | null;
  @ApiProperty({ required: false, nullable: true }) direccion_principal: string | null;
  @ApiProperty({ required: false, nullable: true }) direccion_secundaria: string | null;
  @ApiProperty({ required: false, nullable: true }) referencias_direccion: string | null;
  @ApiProperty({ required: false, nullable: true }) lat: number | null;
  @ApiProperty({ required: false, nullable: true }) lng: number | null;
  @ApiProperty({ required: false, nullable: true }) notas_adicionales: string | null;
  @ApiProperty({ required: false, nullable: true }) url_google_maps: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class VehiculoResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty() placa: string;
  @ApiProperty() tipo: string;
  @ApiProperty({ required: false, nullable: true }) marca: string | null;
  @ApiProperty({ required: false, nullable: true }) modelo: string | null;
  @ApiProperty({ required: false, nullable: true }) anio: number | null;
  @ApiProperty() estado: string;
  @ApiProperty({ required: false, nullable: true }) color: string | null;
  @ApiProperty({ required: false, nullable: true }) capacidad_carga_kg: number | null;
  @ApiProperty({ required: false, nullable: true }) numero_chasis: string | null;
  @ApiProperty({ required: false, nullable: true }) numero_motor: string | null;
  @ApiProperty({ required: false, nullable: true }) tipo_combustible: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'date' }) vencimiento_seguro: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'date' }) vencimiento_circulacion: string | null;
  @ApiProperty({ required: false, nullable: true }) notas_adicionales: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) conductor_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) ruta_activa_id: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class ParadaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty({ format: 'uuid' }) ruta_id: string;
  @ApiProperty() orden: number;
  @ApiProperty() nombre: string;
  @ApiProperty({ required: false, nullable: true }) direccion: string | null;
  @ApiProperty({ required: false, nullable: true }) lat: number | null;
  @ApiProperty({ required: false, nullable: true }) lng: number | null;
  @ApiProperty() completada: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) pedido_id: string | null;
  @ApiProperty() tipo: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) cliente_id: string | null;
  @ApiProperty({ required: false, nullable: true }) telefono: string | null;
  @ApiProperty({ required: false, nullable: true }) referencias: string | null;
  @ApiProperty({ required: false, nullable: true }) horario_atencion: string | null;
  @ApiProperty({ required: false, nullable: true }) notas: string | null;
  @ApiProperty({ required: false, nullable: true }) tipo_combustible: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class RutaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty() nombre: string;
  @ApiProperty({ required: false, nullable: true }) descripcion: string | null;
  @ApiProperty() estado: string;
  @ApiProperty({ type: [String] }) personal_asignado_ids: string[];
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) vehiculo_asignado_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) fecha_inicio: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) fecha_fin: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
  @ApiProperty({ type: () => [ParadaResponseDto], required: false }) paradas?: ParadaResponseDto[];
}

export class PedidoResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty() cliente: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) cliente_id: string | null;
  @ApiProperty({ required: false, nullable: true }) direccion_entrega: string | null;
  @ApiProperty({ required: false, nullable: true }) lat: number | null;
  @ApiProperty({ required: false, nullable: true }) lng: number | null;
  @ApiProperty() estado: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) ruta_id: string | null;
  @ApiProperty({ required: false, nullable: true }) notas: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class AsistenciaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty({ format: 'uuid' }) personal_id: string;
  @ApiProperty() tipo: string;
  @ApiProperty({ format: 'date-time' }) registrado_en: string;
  @ApiProperty({ required: false, nullable: true }) lat: number | null;
  @ApiProperty({ required: false, nullable: true }) lng: number | null;
  @ApiProperty() estado_puntualidad: string;
  @ApiProperty() en_sede: boolean;
  @ApiProperty({ required: false, nullable: true }) hora_real_llegada: string | null;
  @ApiProperty({ required: false, nullable: true }) minutos_retraso_real: number | null;
  @ApiProperty() monitoreo_activo: boolean;
  @ApiProperty({ required: false, nullable: true }) notas: string | null;
  @ApiProperty({ required: false, nullable: true }) justificacion: string | null;
  @ApiProperty({ type: [Object] }) fotos_justificacion: Record<string, unknown>[];
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class IncidenciaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty() tipo: string;
  @ApiProperty() estado: string;
  @ApiProperty() descripcion: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) personal_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) vehiculo_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) ruta_id: string | null;
  @ApiProperty({ required: false, nullable: true }) resolucion: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class TrackingResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty({ format: 'uuid' }) personal_id: string;
  @ApiProperty() latitud: number;
  @ApiProperty() longitud: number;
  @ApiProperty({ required: false, nullable: true }) precision: number | null;
  @ApiProperty({ required: false, nullable: true }) velocidad: number | null;
  @ApiProperty({ required: false, nullable: true }) rumbo: number | null;
  @ApiProperty({ format: 'date-time' }) registrado_en: string;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class VisitaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty({ format: 'uuid' }) personal_id: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) cliente_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) ruta_id: string | null;
  @ApiProperty() tipo_actividad: string;
  @ApiProperty() resultado: string;
  @ApiProperty({ required: false, nullable: true }) lat: number | null;
  @ApiProperty({ required: false, nullable: true }) lng: number | null;
  @ApiProperty({ required: false, nullable: true }) distancia_al_cliente: number | null;
  @ApiProperty({ required: false, nullable: true }) hora_llegada: string | null;
  @ApiProperty({ required: false, nullable: true }) hora_salida: string | null;
  @ApiProperty({ required: false, nullable: true }) duracion_minutos: number | null;
  @ApiProperty({ required: false, nullable: true }) notas: string | null;
  @ApiProperty() synced: boolean;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class CumplimientoResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) tenant_id: string;
  @ApiProperty({ format: 'uuid' }) ruta_id: string;
  @ApiProperty({ format: 'date' }) fecha: string;
  @ApiProperty({ type: Object }) metricas: Record<string, unknown>;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) deleted_at: string | null;
  @ApiProperty({ format: 'date-time' }) created_at: string;
  @ApiProperty({ format: 'date-time' }) updated_at: string;
}

export class UbicacionResponseDto {
  @ApiProperty({ format: 'uuid' }) personalId: string;
  @ApiProperty({ nullable: true }) latitud: number | null;
  @ApiProperty({ nullable: true }) longitud: number | null;
  @ApiProperty({ nullable: true }) precision: number | null;
  @ApiProperty({ nullable: true, format: 'date-time' }) timestamp: string | null;
}

export class SyncResultadoResponseDto {
  @ApiProperty() tipo: string;
  @ApiProperty({ required: false, format: 'uuid' }) id?: string;
  @ApiProperty() ok: boolean;
  @ApiProperty({ required: false }) error?: string;
}
