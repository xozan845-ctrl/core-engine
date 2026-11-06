import { ApiProperty } from '@nestjs/swagger';
import {
  IsDateString,
  IsIn,
  IsInt,
  IsObject,
  IsOptional,
  IsString,
  IsUUID,
  Max,
  Min,
} from 'class-validator';

export class CrearRecursoRequestDto {
  @ApiProperty({ enum: ['profesional', 'sala', 'sillon', 'vehiculo', 'otro'] })
  @IsIn(['profesional', 'sala', 'sillon', 'vehiculo', 'otro'])
  tipo: 'profesional' | 'sala' | 'sillon' | 'vehiculo' | 'otro';

  @ApiProperty({ example: 'Dra. Ana López' }) @IsString() nombre: string;

  @ApiProperty({ required: false, type: Object })
  @IsOptional() @IsObject() atributos?: Record<string, unknown>;
}

export class CrearDisponibilidadRequestDto {
  @ApiProperty({ format: 'uuid' }) @IsUUID() recurso_id: string;
  @ApiProperty({ minimum: 0, maximum: 6, description: '0 domingo, 6 sábado' })
  @IsInt() @Min(0) @Max(6) dia_semana: number;
  @ApiProperty({ example: '08:00' }) @IsString() inicia_hora: string;
  @ApiProperty({ example: '17:00' }) @IsString() termina_hora: string;
  @ApiProperty({ required: false, example: 'America/Managua' }) @IsOptional() @IsString() zona_horaria?: string;
  @ApiProperty({ required: false, format: 'date' }) @IsOptional() @IsDateString() vigente_desde?: string;
  @ApiProperty({ required: false, format: 'date' }) @IsOptional() @IsDateString() vigente_hasta?: string;
}

export class CrearBloqueoRequestDto {
  @ApiProperty({ format: 'uuid' }) @IsUUID() recurso_id: string;
  @ApiProperty({ format: 'date-time' }) @IsDateString() inicia_en: string;
  @ApiProperty({ format: 'date-time' }) @IsDateString() termina_en: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() motivo?: string;
}

export class CrearCitaRequestDto {
  @ApiProperty({ format: 'uuid' }) @IsUUID() recurso_id: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() cliente_id?: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() order_id?: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() item_id?: string;
  @ApiProperty({ format: 'date-time' }) @IsDateString() inicia_en: string;
  @ApiProperty({ format: 'date-time' }) @IsDateString() termina_en: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notas?: string;
  @ApiProperty({ required: false, type: Object }) @IsOptional() @IsObject() metadata?: Record<string, unknown>;
}
