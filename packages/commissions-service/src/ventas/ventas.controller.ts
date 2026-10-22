import { Controller, Get } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiTags } from '@nestjs/swagger';
import {
  Roles,
  ROLES,
  UsuarioActual,
  UsuarioContexto,
  NotFoundError,
} from '@core/shared';
import { VentasService, EstadoCuenta, EstadoCuentaEmpresa } from './ventas.service';
import { EstadoCuentaEmpresaResponseDto, EstadoCuentaResponseDto } from './ventas.response.dto';

/** GET /api/v1/vendedores/me/ventas — ordenes y comisiones del vendedor (Tabla 21). */
@ApiTags('Comisiones · Ventas')
@ApiBearerAuth()
@Controller('api/v1/vendedores/me')
export class VentasController {
  constructor(private readonly ventas: VentasService) {}

  @Get('ventas')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Estado de cuenta de ventas/comisiones del vendedor' })
  @ApiOkResponse({ type: EstadoCuentaResponseDto, description: 'Estado de cuenta del vendedor' })
  async estado(@UsuarioActual() usuario: UsuarioContexto): Promise<EstadoCuenta> {
    return this.ventas.estadoCuenta(usuario.user_id);
  }
}

/** GET /api/v1/empresarios/me/ventas — estado de cuenta de la empresa (ADR-25). */
@ApiTags('Comisiones · Ventas')
@ApiBearerAuth()
@Controller('api/v1/empresarios/me')
export class VentasEmpresaController {
  constructor(private readonly ventas: VentasService) {}

  @Get('ventas')
  @Roles(ROLES.EMPRESARIO)
  @ApiOperation({ summary: 'Estado de cuenta de ventas/comisiones de la empresa' })
  @ApiOkResponse({
    type: EstadoCuentaEmpresaResponseDto,
    description: 'Estado de cuenta de la empresa',
  })
  async estado(@UsuarioActual() usuario: UsuarioContexto): Promise<EstadoCuentaEmpresa> {
    if (!usuario.tenant_id) throw new NotFoundError('Empresa', 'activa');
    return this.ventas.estadoCuentaEmpresa(usuario.tenant_id);
  }
}
