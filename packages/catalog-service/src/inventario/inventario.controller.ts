import { Body, Controller, Get, Post, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiQuery, ApiTags, ApiProperty } from '@nestjs/swagger';
import { IsDateString, IsInt, IsOptional, IsString, IsUUID, Min } from 'class-validator';
import { InventarioService, LineaInventario, LoteInventario, ProveedorInventario } from './inventario.service';
import {
  LineaInventarioResponseDto,
  LoteInventarioResponseDto,
  ProveedorInventarioResponseDto,
} from './inventario.response.dto';
import { CAPACIDADES, RequiereCapacidades, Roles, ROLES } from '@core/shared';

export class CrearProveedorInventarioRequestDto {
  @ApiProperty({ format: 'uuid' }) @IsUUID() empresa_id: string;
  @ApiProperty() @IsString() codigo: string;
  @ApiProperty() @IsString() nombre: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() correo?: string;
  @ApiProperty({ required: false }) @IsOptional() @IsString() telefono?: string;
}

export class CrearLoteInventarioRequestDto {
  @ApiProperty({ format: 'uuid' }) @IsUUID() empresa_id: string;
  @ApiProperty({ format: 'uuid' }) @IsUUID() item_id: string;
  @ApiProperty({ required: false, format: 'uuid' }) @IsOptional() @IsUUID() proveedor_id?: string;
  @ApiProperty() @IsString() codigo_lote: string;
  @ApiProperty({ required: false, format: 'date' }) @IsOptional() @IsDateString() vence_en?: string;
  @ApiProperty({ minimum: 0 }) @IsInt() @Min(0) cantidad_inicial: number;
}

@ApiTags('Catálogo')
@ApiBearerAuth()
@Controller('api/v1/admin/inventario')
@RequiereCapacidades(CAPACIDADES.INVENTARIO_STOCK)
export class InventarioController {
  constructor(private readonly inventario: InventarioService) {}

  /** GET /api/v1/admin/inventario — niveles de stock (JWT admin, Tabla 21). */
  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'Niveles de stock (admin; Tabla 21)' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Filtra el inventario de una empresa (ADR-27)' })
  @ApiOkResponse({ type: [LineaInventarioResponseDto], description: 'Líneas de inventario' })
  async listar(@Query('empresa_id') empresaId?: string): Promise<LineaInventario[]> {
    return this.inventario.listar(empresaId);
  }

  @Post('proveedores')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @RequiereCapacidades(CAPACIDADES.INVENTARIO_LOTES)
  @ApiOperation({ summary: 'Crear proveedor de inventario' })
  @ApiCreatedResponse({ type: ProveedorInventarioResponseDto, description: 'Proveedor creado' })
  async crearProveedor(@Body() dto: CrearProveedorInventarioRequestDto): Promise<ProveedorInventario> {
    return this.inventario.crearProveedor(dto);
  }

  @Post('lotes')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @RequiereCapacidades(CAPACIDADES.INVENTARIO_LOTES)
  @ApiOperation({ summary: 'Registrar lote y vencimiento de un item' })
  @ApiCreatedResponse({ type: LoteInventarioResponseDto, description: 'Lote creado' })
  async crearLote(@Body() dto: CrearLoteInventarioRequestDto): Promise<LoteInventario> {
    return this.inventario.crearLote(dto);
  }

  @Get('lotes')
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @RequiereCapacidades(CAPACIDADES.INVENTARIO_LOTES)
  @ApiOperation({ summary: 'Listar lotes de inventario' })
  @ApiOkResponse({ type: [LoteInventarioResponseDto], description: 'Lotes activos' })
  async listarLotes(@Query('empresa_id') empresaId: string, @Query('item_id') itemId?: string): Promise<LoteInventario[]> {
    return this.inventario.listarLotes(empresaId, itemId);
  }
}
