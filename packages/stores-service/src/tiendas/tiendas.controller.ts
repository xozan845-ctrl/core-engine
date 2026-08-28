import { Body, Controller, Get, Post } from '@nestjs/common';
import { IsOptional, IsString, Length } from 'class-validator';
import { TiendasService, Tienda } from './tiendas.service';
import { Roles, ROLES, UsuarioActual, UsuarioContexto, NotFoundError } from '@core/shared';

export class CrearTiendaDto {
  @IsString()
  @Length(2, 100)
  nombre: string;

  @IsOptional()
  @IsString()
  descripcion?: string;
}

@Controller('api/v1/vendedores')
export class TiendasController {
  constructor(private readonly tiendas: TiendasService) {}

  @Post('tienda')
  @Roles(ROLES.VENDEDOR)
  async crear(@Body() dto: CrearTiendaDto, @UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    return this.tiendas.crear(usuario.user_id, dto.nombre, dto.descripcion);
  }

  @Get('me/tienda')
  @Roles(ROLES.VENDEDOR)
  async miTienda(@UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    const tienda = await this.tiendas.deVendedor(usuario.user_id);
    if (!tienda) throw new NotFoundError('Tienda', 'del vendedor');
    return tienda;
  }
}