import { Body, Controller, Get, Post } from '@nestjs/common';
import { ApiOperation, ApiProperty, ApiResponse, ApiTags } from '@nestjs/swagger';
import { IsInt, IsOptional, IsPositive } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { requerirTenant } from '../tenant';
import { BucketsService, EstadoBucket } from './buckets.service';

export class ConfigurarBucketRequestDto {
  @ApiProperty({
    description: 'Cuota del tenant en bytes (por defecto la del entorno)',
    required: false,
    example: 5368709120,
  })
  @IsOptional()
  @IsInt()
  @IsPositive()
  cuota_bytes?: number;
}

@ApiTags('Almacenamiento')
@Controller('api/v1/storage')
export class BucketsController {
  constructor(private readonly buckets: BucketsService) {}

  @Get('bucket')
  @ApiOperation({
    summary: 'Estado del almacenamiento del tenant (bucket, cuota, uso)',
    description: 'Crea el bucket del tenant de forma perezosa la primera vez.',
  })
  async estado(@UsuarioActual() usuario: UsuarioContexto): Promise<EstadoBucket> {
    return this.buckets.asegurar(requerirTenant(usuario));
  }

  @Post('bucket')
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Asegurar el bucket del tenant y fijar su cuota (admin)' })
  @ApiResponse({ status: 400, description: 'Cuota invalida (CUOTA_INVALIDA/CUOTA_MENOR_AL_USO)' })
  async configurar(
    @Body() dto: ConfigurarBucketRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<EstadoBucket> {
    const tenantId = requerirTenant(usuario);
    const estado = await this.buckets.asegurar(tenantId);
    if (dto.cuota_bytes === undefined) return estado;
    return this.buckets.actualizarCuota(tenantId, dto.cuota_bytes);
  }
}
