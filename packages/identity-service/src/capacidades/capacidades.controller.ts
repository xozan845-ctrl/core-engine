import { Body, Controller, Get, Param, Patch } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiTags } from '@nestjs/swagger';
import { IsBoolean, IsObject, IsOptional } from 'class-validator';
import { ROLES, Roles, UsuarioActual, UsuarioContexto } from '@core/shared';
import { CapacidadesService, EmpresaCapacidad } from './capacidades.service';

class ActualizarCapacidadRequestDto {
  @ApiProperty({ description: 'Activa o desactiva la capacidad' })
  @IsBoolean()
  activo: boolean;

  @ApiProperty({ required: false, type: Object, description: 'Configuracion especifica de la capacidad' })
  @IsOptional()
  @IsObject()
  config?: Record<string, unknown>;
}

class EmpresaCapacidadResponseDto implements EmpresaCapacidad {
  @ApiProperty() capacidad_codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() modulo: string;
  @ApiProperty() activo: boolean;
  @ApiProperty({ type: Object }) config: Record<string, unknown>;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

@ApiTags('Capacidades')
@ApiBearerAuth()
@Controller('api/v2/empresas/:empresaId/capacidades')
export class CapacidadesController {
  constructor(private readonly capacidades: CapacidadesService) {}

  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar capacidades configuradas de una empresa' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiOkResponse({ type: [EmpresaCapacidadResponseDto], description: 'Capacidades de la empresa' })
  listar(@Param('empresaId') empresaId: string, @UsuarioActual() actor: UsuarioContexto) {
    return this.capacidades.listar(empresaId, actor);
  }

  @Patch(':capacidad')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Activar o desactivar una capacidad de la empresa' })
  @ApiParam({ name: 'empresaId', format: 'uuid' })
  @ApiParam({ name: 'capacidad', description: 'Codigo estable de capacidad' })
  @ApiOkResponse({ type: EmpresaCapacidadResponseDto, description: 'Capacidad actualizada' })
  actualizar(
    @Param('empresaId') empresaId: string,
    @Param('capacidad') capacidad: string,
    @Body() dto: ActualizarCapacidadRequestDto,
    @UsuarioActual() actor: UsuarioContexto,
  ) {
    return this.capacidades.actualizar(empresaId, capacidad.trim().toLowerCase(), dto.activo, dto.config, actor);
  }
}
