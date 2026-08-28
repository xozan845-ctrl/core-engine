import { Controller, Get } from '@nestjs/common';
import {
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { VentasService, EstadoCuenta } from './ventas.service';

/** GET /api/v1/vendedores/me/ventas — ordenes y comisiones del vendedor (Tabla 21). */
@Controller('api/v1/vendedores/me')
export class VentasController {
  constructor(private readonly ventas: VentasService) {}

  @Get('ventas')
  @Roles(ROLES.VENDEDOR)
  async estado(@UsuarioActual() usuario: UsuarioContexto): Promise<EstadoCuenta> {
    return this.ventas.estadoCuenta(usuario.user_id);
  }
}