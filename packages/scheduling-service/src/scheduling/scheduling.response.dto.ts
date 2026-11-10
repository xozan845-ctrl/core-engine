import { ApiProperty } from '@nestjs/swagger';

export class RecursoResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ enum: ['profesional', 'sala', 'sillon', 'vehiculo', 'otro'] }) tipo: string;
  @ApiProperty() nombre: string;
  @ApiProperty({ enum: ['activo', 'inactivo'] }) estado: string;
  @ApiProperty({ type: Object }) atributos: Record<string, unknown>;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class DisponibilidadResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) recurso_id: string;
  @ApiProperty() dia_semana: number;
  @ApiProperty() inicia_hora: string;
  @ApiProperty() termina_hora: string;
  @ApiProperty() zona_horaria: string;
  @ApiProperty({ format: 'date' }) vigente_desde: string;
  @ApiProperty({ required: false, nullable: true, format: 'date' }) vigente_hasta: string | null;
}

export class CitaResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) recurso_id: string;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) cliente_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) order_id: string | null;
  @ApiProperty({ required: false, nullable: true, format: 'uuid' }) item_id: string | null;
  @ApiProperty({ enum: ['reservada', 'confirmada', 'en_atencion', 'completada', 'cancelada', 'no_asistio'] }) estado: string;
  @ApiProperty({ format: 'date-time' }) inicia_en: string;
  @ApiProperty({ format: 'date-time' }) termina_en: string;
  @ApiProperty() notas: string;
  @ApiProperty({ type: Object }) metadata: Record<string, unknown>;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class BloqueoResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) recurso_id: string;
  @ApiProperty({ format: 'date-time' }) inicia_en: string;
  @ApiProperty({ format: 'date-time' }) termina_en: string;
  @ApiProperty() motivo: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
}

export class RecordatorioResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) cita_id: string;
  @ApiProperty({ enum: ['24h', '2h'] }) tipo: string;
  @ApiProperty({ format: 'date-time' }) programado_en: string;
  @ApiProperty({ enum: ['pendiente', 'enviado', 'cancelado'] }) estado: string;
  @ApiProperty({ required: false, nullable: true, format: 'date-time' }) enviado_en: string | null;
}
