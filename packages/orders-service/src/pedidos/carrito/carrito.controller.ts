import {
  Body,
  Controller,
  Delete,
  Get,
  Param,
  Patch,
  Post,
} from '@nestjs/common';
import { Roles, ROLES, UsuarioActual, UsuarioContexto } from '@core/shared';
import { CarritoService, CarritoVista } from './carrito.service';
import { AgregarItemRequestDto, ActualizarCantidadRequestDto } from './carrito.dtos';

/**
 * Carrito del comprador (RN-05): expira tras 30 minutos de inactividad y no
 * reserva stock (RN-03). El checkout crea la orden desde este carrito.
 */
@Controller('api/v1/carrito')
export class CarritoController {
  constructor(private readonly carritos: CarritoService) {}

  @Get()
  @Roles(ROLES.COMPRADOR)
  async ver(@UsuarioActual() usuario: UsuarioContexto): Promise<CarritoVista> {
    return this.carritos.obtener(usuario.user_id);
  }

  @Post('items')
  @Roles(ROLES.COMPRADOR)
  async agregar(
    @Body() dto: AgregarItemRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<CarritoVista> {
    return this.carritos.agregar(usuario.user_id, dto.oferta_id, dto.cantidad);
  }

  @Patch('items/:ofertaId')
  @Roles(ROLES.COMPRADOR)
  async actualizar(
    @Param('ofertaId') ofertaId: string,
    @Body() dto: ActualizarCantidadRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<CarritoVista> {
    return this.carritos.actualizarCantidad(usuario.user_id, ofertaId, dto.cantidad);
  }

  @Delete('items/:ofertaId')
  @Roles(ROLES.COMPRADOR)
  async quitar(
    @Param('ofertaId') ofertaId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<CarritoVista> {
    return this.carritos.quitar(usuario.user_id, ofertaId);
  }

  @Delete()
  @Roles(ROLES.COMPRADOR)
  async vaciar(@UsuarioActual() usuario: UsuarioContexto): Promise<CarritoVista> {
    return this.carritos.vaciar(usuario.user_id);
  }
}
