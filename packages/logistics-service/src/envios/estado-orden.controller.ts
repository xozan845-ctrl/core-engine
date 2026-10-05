import {
  Body,
  Controller,
  Get,
  Param,
  Patch,
} from '@nestjs/common';
import { ApiOperation, ApiParam, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import { IsIn, IsOptional, IsString } from 'class-validator';
import {
  Roles,
  ROLES,
  DomainError,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { EnviosService, Envio, OrdenExterna } from './envios.service';

export class AvanzarEstadoRequestDto {
  @ApiProperty({
    description: 'Estado objetivo del ciclo de vida de la orden',
    enum: ['en_preparacion', 'enviada', 'entregada', 'cancelada', 'devuelta'],
    example: 'enviada',
  })
  @IsIn(['en_preparacion', 'enviada', 'entregada', 'cancelada', 'devuelta'])
  estado: string;

  @ApiProperty({ description: 'Motivo del cambio de estado', required: false, example: 'despacho a ruta 3' })
  @IsOptional()
  @IsString()
  motivo?: string;
}

/**
 * PATCH /api/v1/orders/:id/estado — el servicio de logistica avanza el ciclo de
 * vida de la orden (Tabla 21: JWT admin/logistica). La escritura real vive en
 * el orders-service (Event Sourcing) via endpoint interno.
 */
@ApiTags('Logística')
@Controller('api/v1/orders')
export class EstadoOrdenController {
  constructor(private readonly envios: EnviosService) {}

  @Patch(':id/estado')
  @Roles(ROLES.ADMIN, ROLES.LOGISTICA)
  @ApiOperation({ summary: 'Avanzar el estado de una orden (logística)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  @ApiResponse({ status: 200, description: 'Orden con el nuevo estado' })
  @ApiResponse({ status: 400, description: 'Estado objetivo inválido (ESTADO_INVALIDO)' })
  async avanzar(
    @Param('id') id: string,
    @Body() dto: AvanzarEstadoRequestDto,
    @UsuarioActual() _usuario: UsuarioContexto,
  ): Promise<OrdenExterna> {
    if (!['en_preparacion', 'enviada', 'entregada', 'cancelada', 'devuelta'].includes(dto.estado)) {
      throw new DomainError('ESTADO_INVALIDO', 'Estado objetivo invalido para logistica.');
    }
    return this.envios.avanzarEstadoOrden(id, dto.estado, dto.motivo);
  }
}

/** GET /api/v1/admin/envios — guias de despacho (JWT admin). */
@ApiTags('Logística')
@Controller('api/v1/admin')
export class EnviosAdminController {
  constructor(private readonly envios: EnviosService) {}

  @Get('envios')
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Listar guías de despacho (admin)' })
  @ApiResponse({ status: 200, description: 'Guías de despacho con monto serializado' })
  async listar(): Promise<(Envio & { monto: string })[]> {
    return this.envios.listar();
  }
}
