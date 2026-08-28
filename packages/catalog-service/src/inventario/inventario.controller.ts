import { Controller, Get } from '@nestjs/common';
import { InventarioService, LineaInventario } from './inventario.service';
import { Roles, ROLES } from '@core/shared';

@Controller('api/v1/admin/inventario')
export class InventarioController {
  constructor(private readonly inventario: InventarioService) {}

  /** GET /api/v1/admin/inventario — niveles de stock (JWT admin, Tabla 21). */
  @Get()
  @Roles(ROLES.ADMIN)
  async listar(): Promise<LineaInventario[]> {
    return this.inventario.listar();
  }
}