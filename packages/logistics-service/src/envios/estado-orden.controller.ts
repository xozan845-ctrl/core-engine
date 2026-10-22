import {
  Body,
  Controller,
  Get,
  Param,
  Patch,
  Post,
} from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import { IsIn, IsOptional, IsString } from 'class-validator';
import {
  Roles,
  ROLES,
  DomainError,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { EnviosService, Envio, OrdenExterna, ProveedorTipo } from './envios.service';
import { EnvioConMontoResponseDto, EnvioResponseDto, OrdenExternaResponseDto } from './envios.response.dto';

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
@ApiBearerAuth()
@Controller('api/v1/orders')
export class EstadoOrdenController {
  constructor(private readonly envios: EnviosService) {}

  @Patch(':id/estado')
  @Roles(ROLES.SUPER_ADMIN, ROLES.EMPRESARIO, ROLES.ADMIN, ROLES.LOGISTICA)
  @ApiOperation({ summary: 'Avanzar el estado de una orden (logística o empresario de la empresa)' })
  @ApiParam({ name: 'id', description: 'Id de la orden' })
  @ApiOkResponse({ type: OrdenExternaResponseDto, description: 'Orden con el nuevo estado' })
  @ApiResponse({ status: 400, description: 'Estado objetivo inválido (ESTADO_INVALIDO)' })
  async avanzar(
    @Param('id') id: string,
    @Body() dto: AvanzarEstadoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<OrdenExterna> {
    if (!['en_preparacion', 'enviada', 'entregada', 'cancelada', 'devuelta'].includes(dto.estado)) {
      throw new DomainError('ESTADO_INVALIDO', 'Estado objetivo invalido para logistica.');
    }
    if (usuario.rol === ROLES.EMPRESARIO) {
      if (!usuario.tenant_id) {
        throw new DomainError('TENANT_REQUERIDO', 'Se requiere una empresa activa.');
      }
      return this.envios.avanzarEstadoOrden(id, dto.estado, dto.motivo, usuario.tenant_id);
    }
    return this.envios.avanzarEstadoOrden(id, dto.estado, dto.motivo);
  }
}

/** GET /api/v1/admin/envios — guias de despacho (JWT admin). */
@ApiTags('Logística')
@ApiBearerAuth()
@Controller('api/v1/admin')
export class EnviosAdminController {
  constructor(private readonly envios: EnviosService) {}

  @Get('envios')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Listar guías de despacho (admin)' })
  @ApiOkResponse({ type: [EnvioConMontoResponseDto], description: 'Guías de despacho con monto serializado' })
  async listar(): Promise<(Envio & { monto: string })[]> {
    return this.envios.listar();
  }
}

export class AsignarProveedorRequestDto {
  @ApiProperty({ enum: ['plataforma', 'empresa'], description: 'Flota que atenderá el envío híbrido' })
  @IsIn(['plataforma', 'empresa'])
  proveedor_tipo: ProveedorTipo;
}

export class AvanzarEnvioRequestDto {
  @ApiProperty({ enum: ['enviada', 'entregada', 'cancelada'] })
  @IsIn(['enviada', 'entregada', 'cancelada'])
  estado: 'enviada' | 'entregada' | 'cancelada';

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  motivo?: string;
}

@ApiTags('Logística empresarial')
@ApiBearerAuth()
@Controller('api/v1/empresarios/me/envios')
export class EnviosEmpresaController {
  constructor(private readonly envios: EnviosService) {}

  @Get()
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar envíos de la empresa activa' })
  @ApiOkResponse({ type: [EnvioResponseDto] })
  listar(@UsuarioActual() usuario: UsuarioContexto): Promise<Envio[]> {
    if (!usuario.tenant_id) throw new DomainError('TENANT_REQUERIDO', 'Se requiere una empresa activa.');
    return this.envios.deEmpresa(usuario.tenant_id);
  }

  @Post(':id/asignacion')
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Elegir flota para un envío de modalidad híbrida' })
  @ApiParam({ name: 'id', description: 'Id del envío' })
  @ApiOkResponse({ type: EnvioResponseDto })
  asignar(
    @Param('id') id: string,
    @Body() dto: AsignarProveedorRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Envio> {
    if (!usuario.tenant_id) throw new DomainError('TENANT_REQUERIDO', 'Se requiere una empresa activa.');
    return this.envios.asignarProveedor(id, usuario.tenant_id, dto.proveedor_tipo, usuario.user_id);
  }
}

@ApiTags('Logística')
@ApiBearerAuth()
@Controller('api/v1/envios')
export class EnviosFlotaController {
  constructor(private readonly envios: EnviosService) {}

  @Patch(':id/estado')
  @Roles(ROLES.LOGISTICA, ROLES.COORDINADOR, ROLES.SUPERVISOR, ROLES.OPERATIVO)
  @ApiOperation({ summary: 'Avanzar un envío asignado a la flota activa' })
  @ApiParam({ name: 'id', description: 'Id del envío' })
  @ApiOkResponse({ type: EnvioResponseDto })
  avanzar(
    @Param('id') id: string,
    @Body() dto: AvanzarEnvioRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Envio> {
    if (!usuario.tenant_id) throw new DomainError('TENANT_REQUERIDO', 'Se requiere el tenant de la flota.');
    return this.envios.avanzarEstadoEnvio(id, usuario.tenant_id, dto.estado, dto.motivo);
  }
}
