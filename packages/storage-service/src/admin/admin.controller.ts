import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiTags } from '@nestjs/swagger';
import { IsIn, IsInt, IsOptional, IsPositive, Max } from 'class-validator';
import { ROLES, Roles } from '@core/shared';
import { BucketsService, EstadoAlmacen, EstadoBucket } from '../buckets/buckets.service';
import {
  EstadoBucketResponseDto,
  PaginaBucketsResponseDto,
} from '../buckets/buckets.response.dto';

export class PaginacionRequestDto {
  @ApiProperty({ description: 'Pagina (1-based)', required: false, minimum: 1 })
  @IsOptional()
  @IsPositive()
  pagina?: number;

  @ApiProperty({ description: 'Tamano de pagina (max 100)', required: false, minimum: 1, maximum: 100 })
  @IsOptional()
  @IsPositive()
  @Max(100)
  limite?: number;
}

export class ActualizarTenantRequestDto {
  @ApiProperty({ description: 'Nueva cuota en bytes', required: false })
  @IsOptional()
  @IsInt()
  @IsPositive()
  cuota_bytes?: number;

  @ApiProperty({ description: 'Estado del almacenamiento del tenant', required: false, enum: ['activo', 'suspendido'] })
  @IsOptional()
  @IsIn(['activo', 'suspendido'])
  estado?: EstadoAlmacen;
}

/**
 * Operacion multi-tenant (admin): inventario de buckets, cuotas y estado por
 * organizacion. Las rutas viven bajo `/api/v1/admin/...`, que el gateway ya
 * restringe al rol admin (R-DS-3).
 */
@ApiTags('Almacenamiento (admin)')
@ApiBearerAuth()
@Controller('api/v1/admin/storage')
@Roles(ROLES.ADMIN)
export class AdminStorageController {
  constructor(private readonly buckets: BucketsService) {}

  @Get('tenants')
  @ApiOperation({ summary: 'Listar tenants con almacenamiento (cuota y uso)' })
  @ApiOkResponse({ type: PaginaBucketsResponseDto, description: 'Página de tenants' })
  async listar(@Query() query: PaginacionRequestDto) {
    return this.buckets.listar(query.pagina, query.limite);
  }

  @Get('tenants/:tenantId')
  @ApiOperation({ summary: 'Estado de almacenamiento de un tenant' })
  @ApiParam({ name: 'tenantId', description: 'Id del tenant (organizacion)' })
  @ApiOkResponse({ type: EstadoBucketResponseDto, description: 'Estado del tenant' })
  async estado(@Param('tenantId') tenantId: string): Promise<EstadoBucket> {
    return this.buckets.estado(tenantId);
  }

  @Patch('tenants/:tenantId')
  @ApiOperation({ summary: 'Ajustar cuota y/o suspender el almacenamiento del tenant' })
  @ApiParam({ name: 'tenantId', description: 'Id del tenant (organizacion)' })
  @ApiOkResponse({ type: EstadoBucketResponseDto, description: 'Tenant actualizado' })
  async actualizar(
    @Param('tenantId') tenantId: string,
    @Body() dto: ActualizarTenantRequestDto,
  ): Promise<EstadoBucket> {
    if (dto.cuota_bytes !== undefined) {
      await this.buckets.actualizarCuota(tenantId, dto.cuota_bytes);
    }
    if (dto.estado !== undefined) {
      await this.buckets.cambiarEstado(tenantId, dto.estado);
    }
    return this.buckets.estado(tenantId);
  }

  @Post('tenants/:tenantId/recalcular')
  @ApiOperation({ summary: 'Recalcular el uso del tenant desde la metadata (idempotente)' })
  @ApiParam({ name: 'tenantId', description: 'Id del tenant (organizacion)' })
  @ApiCreatedResponse({ type: EstadoBucketResponseDto, description: 'Uso recalculado' })
  async recalcular(@Param('tenantId') tenantId: string): Promise<EstadoBucket> {
    return this.buckets.recalcularUso(tenantId);
  }
}
