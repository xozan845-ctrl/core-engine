import { Controller, Get, Param, ParseIntPipe, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiParam, ApiProperty, ApiTags } from '@nestjs/swagger';
import { IsOptional, IsString } from 'class-validator';
import { ROLES, Roles } from '@core/shared';
import { Pack, PacksService } from './packs.service';

class ListarPacksQueryDto {
  @ApiProperty({ required: false, enum: ['retail', 'farmacia', 'belleza', 'dental', 'flota'] })
  @IsOptional()
  @IsString()
  vertical_id?: string;
}

class PackCapacidadResponseDto {
  @ApiProperty() capacidad_codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() modulo: string;
  @ApiProperty() activo: boolean;
  @ApiProperty({ type: Object }) config: Record<string, unknown>;
}

class PackResponseDto implements Pack {
  @ApiProperty() codigo: string;
  @ApiProperty() version: number;
  @ApiProperty() vertical_codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() descripcion: string;
  @ApiProperty({ type: Object }) config: Record<string, unknown>;
  @ApiProperty({ type: [PackCapacidadResponseDto] }) capacidades: PackCapacidadResponseDto[];
}

@ApiTags('Packs')
@ApiBearerAuth()
@Controller('api/v1/packs')
export class PacksController {
  constructor(private readonly packs: PacksService) {}

  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Listar packs activos disponibles para crear empresas' })
  @ApiOkResponse({ type: [PackResponseDto] })
  listar(@Query() query: ListarPacksQueryDto) {
    return this.packs.listar(query.vertical_id?.trim().toLowerCase());
  }

  @Get(':codigo/:version')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Obtener un pack versionado y sus capacidades' })
  @ApiParam({ name: 'codigo', example: 'pack_belleza' })
  @ApiParam({ name: 'version', type: Number, example: 1 })
  @ApiOkResponse({ type: PackResponseDto })
  obtener(@Param('codigo') codigo: string, @Param('version', ParseIntPipe) version: number) {
    return this.packs.obtener(codigo.trim().toLowerCase(), version);
  }
}
