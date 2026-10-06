import { Body, Controller, Get, Post } from '@nestjs/common';
import { ApiOperation, ApiProperty, ApiTags } from '@nestjs/swagger';
import { IsOptional, IsString, Length } from 'class-validator';
import { TiendasService, Tienda } from './tiendas.service';
import { Roles, ROLES, UsuarioActual, UsuarioContexto, NotFoundError } from '@core/shared';

export class CrearTiendaRequestDto {
  @ApiProperty({ description: 'Nombre de la tienda', minLength: 2, maxLength: 100, example: 'Mi tienda' })
  @IsString()
  @Length(2, 100)
  nombre: string;

  @ApiProperty({ description: 'Descripción de la tienda', required: false, example: 'Ropa y accesorios' })
  @IsOptional()
  @IsString()
  descripcion?: string;
}

@ApiTags('Tiendas')
@Controller('api/v1/vendedores')
export class TiendasController {
  constructor(private readonly tiendas: TiendasService) {}

  @Post('tienda')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Crear la tienda del vendedor autenticado' })
  async crear(@Body() dto: CrearTiendaRequestDto, @UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    return this.tiendas.crear(usuario.user_id, dto.nombre, dto.descripcion);
  }

  @Get('me/tienda')
  @Roles(ROLES.VENDEDOR)
  @ApiOperation({ summary: 'Tienda del vendedor autenticado' })
  async miTienda(@UsuarioActual() usuario: UsuarioContexto): Promise<Tienda> {
    const tienda = await this.tiendas.deVendedor(usuario.user_id);
    if (!tienda) throw new NotFoundError('Tienda', 'del vendedor');
    return tienda;
  }
}
