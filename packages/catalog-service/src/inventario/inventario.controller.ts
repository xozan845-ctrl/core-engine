import { Controller, Get } from '@nestjs/common';
import { ApiOperation, ApiTags } from '@nestjs/swagger';
import { InventarioService, LineaInventario } from './inventario.service';
import { Roles, ROLES } from '@core/shared';

@ApiTags('Catálogo')
@Controller('api/v1/admin/inventario')
export class InventarioController {
  constructor(private readonly inventario: InventarioService) {}

  /** GET /api/v1/admin/inventario — niveles de stock (JWT admin, Tabla 21). */
  @Get()
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'Niveles de stock (admin; Tabla 21)' })
  async listar(): Promise<LineaInventario[]> {
    return this.inventario.listar();
  }
}
