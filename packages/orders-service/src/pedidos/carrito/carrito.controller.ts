import {
  Body,
  Controller,
  Delete,
  Get,
  Param,
  Patch,
  Post,
} from '@nestjs/common';
import { ApiBearerAuth, ApiCreatedResponse, ApiOkResponse, ApiOperation, ApiParam, ApiTags } from '@nestjs/swagger';
import { Roles, ROLES, UsuarioActual, UsuarioContexto } from '@core/shared';
import { CarritoService, CarritoVista } from './carrito.service';
import { AgregarItemRequestDto, ActualizarCantidadRequestDto } from './carrito.dtos';
import { CarritoResponseDto } from './carrito.response.dto';

/**
 * Carrito del comprador (RN-05): expira tras 30 minutos de inactividad y no
 * reserva stock (RN-03). El checkout crea la orden desde este carrito.
 */
@ApiTags('Carrito')
@ApiBearerAuth()
@Controller('api/v1/carrito')
export class CarritoController {
  constructor(private readonly carritos: CarritoService) {}

  @Get()
  @Roles(ROLES.COMPRADOR)
  @ApiOperation({ summary: 'Ver el carrito del comprador (RN-05)' })
  @ApiOkResponse({ type: CarritoResponseDto, description: 'Carrito del comprador' })
  async ver(@UsuarioActual() usuario: UsuarioContexto): Promise<CarritoVista> {
    return this.carritos.obtener(usuario.user_id);
  }

  @Post('items')
  @Roles(ROLES.COMPRADOR)
  @ApiOperation({ summary: 'Agregar un item al carrito' })
  @ApiCreatedResponse({ type: CarritoResponseDto, description: 'Item agregado' })
  async agregar(
    @Body() dto: AgregarItemRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<CarritoVista> {
    return this.carritos.agregar(usuario.user_id, dto.oferta_id, dto.cantidad);
  }

  @Patch('items/:ofertaId')
  @Roles(ROLES.COMPRADOR)
  @ApiOperation({ summary: 'Actualizar la cantidad de un item (0 lo elimina)' })
  @ApiParam({ name: 'ofertaId', description: 'Id de la oferta' })
  @ApiOkResponse({ type: CarritoResponseDto, description: 'Carrito actualizado' })
  async actualizar(
    @Param('ofertaId') ofertaId: string,
    @Body() dto: ActualizarCantidadRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<CarritoVista> {
    return this.carritos.actualizarCantidad(usuario.user_id, ofertaId, dto.cantidad);
  }

  @Delete('items/:ofertaId')
  @Roles(ROLES.COMPRADOR)
  @ApiOperation({ summary: 'Quitar un item del carrito' })
  @ApiParam({ name: 'ofertaId', description: 'Id de la oferta' })
  @ApiOkResponse({ type: CarritoResponseDto, description: 'Carrito actualizado' })
  async quitar(
    @Param('ofertaId') ofertaId: string,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<CarritoVista> {
    return this.carritos.quitar(usuario.user_id, ofertaId);
  }

  @Delete()
  @Roles(ROLES.COMPRADOR)
  @ApiOperation({ summary: 'Vaciar el carrito' })
  @ApiOkResponse({ type: CarritoResponseDto, description: 'Carrito vacío' })
  async vaciar(@UsuarioActual() usuario: UsuarioContexto): Promise<CarritoVista> {
    return this.carritos.vaciar(usuario.user_id);
  }
}
