import { Controller, Get, Param, Post, Query, UseGuards } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiQuery, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard } from '@core/shared';
import { BucketsService, EstadoBucket } from '../buckets/buckets.service';
import { ObjetosService, ObjetoStorage } from '../objetos/objetos.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna:
 * consultar el estado de almacenamiento de un tenant y recalcular su uso.
 */
@ApiTags('Almacenamiento (interno)')
@ApiSecurity('internalKey')
@Controller('internal/storage')
@UseGuards(ClaveInternaGuard)
export class InternalStorageController {
  constructor(
    private readonly buckets: BucketsService,
    private readonly objetos: ObjetosService,
  ) {}

  @Get('tenants/:tenantId')
  @ApiOperation({ summary: 'Estado de almacenamiento del tenant (servicio→servicio)' })
  @ApiParam({ name: 'tenantId', description: 'Id del tenant' })
  async estado(@Param('tenantId') tenantId: string): Promise<EstadoBucket> {
    return this.buckets.estado(tenantId);
  }

  @Post('tenants/:tenantId/recalcular')
  @ApiOperation({ summary: 'Recalcular el uso del tenant (servicio→servicio)' })
  @ApiParam({ name: 'tenantId', description: 'Id del tenant' })
  async recalcular(@Param('tenantId') tenantId: string): Promise<EstadoBucket> {
    return this.buckets.recalcularUso(tenantId);
  }

  @Get('objetos/:id')
  @ApiOperation({ summary: 'Metadata de un objeto del tenant (servicio→servicio)' })
  @ApiParam({ name: 'id', description: 'Id del objeto' })
  @ApiQuery({ name: 'tenant_id', required: true, description: 'Tenant dueno del objeto' })
  async objeto(
    @Param('id') id: string,
    @Query('tenant_id') tenantId: string,
  ): Promise<ObjetoStorage> {
    return this.objetos.obtener(tenantId, id);
  }
}
