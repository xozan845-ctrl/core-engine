import { Body, Controller, Delete, Get, Param, Patch, Post, Query } from '@nestjs/common';
import {
  ApiBearerAuth,
  ApiCreatedResponse,
  ApiOkResponse,
  ApiOperation,
  ApiParam,
  ApiProperty,
  ApiQuery,
  ApiTags,
} from '@nestjs/swagger';
import { IsInt, IsUUID, Min } from 'class-validator';
import {
  CAPACIDADES,
  DomainError,
  RequiereCapacidades,
  ROLES,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import { ComponenteItem, ComponentesService } from './componentes.service';

export class CrearComponenteRequestDto {
  @ApiProperty({ format: 'uuid', description: 'Producto que consume el item compuesto' })
  @IsUUID()
  componente_item_id: string;

  @ApiProperty({ minimum: 1, example: 2, description: 'Unidades de stock por unidad vendida' })
  @IsInt()
  @Min(1)
  cantidad: number;
}

export class ActualizarComponenteRequestDto {
  @ApiProperty({ minimum: 1, example: 3 })
  @IsInt()
  @Min(1)
  cantidad: number;
}

export class ComponenteItemResponseDto implements ComponenteItem {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty({ format: 'uuid' }) componente_item_id: string;
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() stock: number;
  @ApiProperty() cantidad: number;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

@ApiTags('Catálogo · Composición')
@ApiBearerAuth()
@Controller('api/v2/catalog/items')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.CATALOGO_ITEMS)
export class ComponentesController {
  constructor(private readonly componentes: ComponentesService) {}

  @Get(':id/componentes')
  @ApiOperation({ summary: 'Listar la composición de un item (receta/kit)' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [ComponenteItemResponseDto], description: 'Componentes del item' })
  listar(
    @Param('id') itemId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.componentes.listar(this.empresaDe(usuario, empresaId), itemId);
  }

  @Post(':id/componentes')
  @ApiOperation({ summary: 'Agregar un componente a la composición' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: ComponenteItemResponseDto, description: 'Componente agregado' })
  crear(
    @Param('id') itemId: string,
    @Body() dto: CrearComponenteRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.componentes.crear(this.empresaDe(usuario, empresaId), itemId, dto);
  }

  @Patch(':id/componentes/:componenteId')
  @ApiOperation({ summary: 'Cambiar la cantidad de un componente' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiParam({ name: 'componenteId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: ComponenteItemResponseDto, description: 'Componente actualizado' })
  actualizar(
    @Param('id') itemId: string,
    @Param('componenteId') componenteId: string,
    @Body() dto: ActualizarComponenteRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.componentes.actualizar(
      this.empresaDe(usuario, empresaId),
      itemId,
      componenteId,
      dto,
    );
  }

  @Delete(':id/componentes/:componenteId')
  @ApiOperation({ summary: 'Quitar un componente de la composición' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiParam({ name: 'componenteId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ description: 'Componente eliminado' })
  eliminar(
    @Param('id') itemId: string,
    @Param('componenteId') componenteId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.componentes.eliminar(this.empresaDe(usuario, empresaId), itemId, componenteId);
  }

  private empresaDe(usuario: UsuarioContexto, solicitada?: string): string {
    if (usuario.rol === ROLES.EMPRESARIO || usuario.rol_empresa_id) {
      if (!usuario.tenant_id) throw new DomainError('EMPRESA_REQUERIDA', 'No hay empresa activa.');
      if (solicitada && solicitada !== usuario.tenant_id) {
        throw new DomainError('EMPRESA_NO_AUTORIZADA', 'No puedes operar otra empresa.');
      }
      return usuario.tenant_id;
    }
    if (!solicitada) throw new DomainError('EMPRESA_REQUERIDA', 'Indica empresa_id.');
    return solicitada;
  }
}
