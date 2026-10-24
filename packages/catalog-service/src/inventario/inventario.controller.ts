import { Controller, Get, Query } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiQuery, ApiTags } from '@nestjs/swagger';
import { InventarioService, LineaInventario } from './inventario.service';
import { LineaInventarioResponseDto } from './inventario.response.dto';
import { Roles, ROLES } from '@core/shared';

@ApiTags('Catálogo')
@ApiBearerAuth()
@Controller('api/v1/admin/inventario')
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
}
