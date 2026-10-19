import { Controller, Get } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiTags } from '@nestjs/swagger';
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
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Niveles de stock (admin; Tabla 21)' })
  @ApiOkResponse({ type: [LineaInventarioResponseDto], description: 'Líneas de inventario' })
  async listar(): Promise<LineaInventario[]> {
    return this.inventario.listar();
  }
}
