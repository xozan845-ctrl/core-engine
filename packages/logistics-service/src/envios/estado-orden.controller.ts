import {
  Body,
  Controller,
  Get,
  Param,
  Patch,
} from '@nestjs/common';
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
  @IsIn(['en_preparacion', 'enviada', 'entregada', 'cancelada', 'devuelta'])
  estado: string;

  @IsOptional()
  @IsString()
  motivo?: string;
}

/**
 * PATCH /api/v1/orders/:id/estado — el servicio de logistica avanza el ciclo de
 * vida de la orden (Tabla 21: JWT admin/logistica). La escritura real vive en
 * el orders-service (Event Sourcing) via endpoint interno.
 */
@Controller('api/v1/orders')
export class EstadoOrdenController {
  constructor(private readonly envios: EnviosService) {}

  @Patch(':id/estado')
  @Roles(ROLES.ADMIN, ROLES.LOGISTICA)
  async avanzar(
    @Param('id') id: string,
    @Body() dto: AvanzarEstadoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrdenExterna> {
    if (!['en_preparacion', 'enviada', 'entregada', 'cancelada', 'devuelta'].includes(dto.estado)) {
      throw new DomainError('ESTADO_INVALIDO', 'Estado objetivo invalido para logistica.');
    }
    return this.envios.avanzarEstadoOrden(id, dto.estado, dto.motivo);
  }
}

/** GET /api/v1/admin/envios — guias de despacho (JWT admin). */
@Controller('api/v1/admin')
export class EnviosAdminController {
  constructor(private readonly envios: EnviosService) {}

  @Get('envios')
  @Roles(ROLES.ADMIN)
  async listar(): Promise<(Envio & { monto: string })[]> {
    return this.envios.listar();
  }
}
