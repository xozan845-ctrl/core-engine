import { Body, Controller, Post, UseGuards } from '@nestjs/common';
import {
  ApiCreatedResponse,
  ApiOperation,
  ApiProperty,
  ApiSecurity,
  ApiTags,
} from '@nestjs/swagger';
import { IsDateString, IsUUID } from 'class-validator';
import { ClaveInternaGuard } from '@core/shared';
import { SchedulingService } from './scheduling.service';

export class VerificarDisponibilidadRequestDto {
  @ApiProperty({ format: 'uuid' }) @IsUUID() empresa_id: string;
  @ApiProperty({ format: 'uuid' }) @IsUUID() recurso_id: string;
  @ApiProperty({ format: 'date-time' }) @IsDateString() inicia_en: string;
  @ApiProperty({ format: 'date-time' }) @IsDateString() termina_en: string;
}

export class DisponibilidadResponseDto {
  @ApiProperty() disponible: boolean;
}

/** Superficie interna para que orders valide una reserva antes de crearla. */
@ApiTags('Scheduling (interno)')
@ApiSecurity('internalKey')
@Controller('internal/scheduling')
@UseGuards(ClaveInternaGuard)
export class InternalSchedulingController {
  constructor(private readonly scheduling: SchedulingService) {}

  @Post('disponibilidad')
  @ApiOperation({ summary: 'Verificar disponibilidad de un recurso en un rango' })
  @ApiCreatedResponse({ type: DisponibilidadResponseDto, description: 'Recurso disponible' })
  async disponibilidad(
    @Body() dto: VerificarDisponibilidadRequestDto,
  ): Promise<DisponibilidadResponseDto> {
    await this.scheduling.verificarDisponibilidad(
      dto.empresa_id,
      dto.recurso_id,
      dto.inicia_en,
      dto.termina_en,
    );
    return { disponible: true };
  }
}
