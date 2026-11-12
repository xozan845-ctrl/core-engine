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
  PartialType,
} from '@nestjs/swagger';
import { IsBoolean, IsInt, IsOptional, IsString, IsUUID, Matches, Min } from 'class-validator';
import {
  CAPACIDADES,
  DomainError,
  RequiereCapacidades,
  ROLES,
  Roles,
  UsuarioActual,
  UsuarioContexto,
} from '@core/shared';
import {
  ListaPrecio,
  PrecioItem,
  PrecioResuelto,
  PreciosService,
} from './precios.service';

export class CrearListaPrecioRequestDto {
  @ApiProperty({ description: 'Codigo estable de la lista', example: 'mayorista' })
  @Matches(/^[a-z][a-z0-9_-]{1,39}$/)
  codigo: string;

  @ApiProperty({ example: 'Mayorista' })
  @IsString()
  nombre: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false, default: 'NIO', example: 'NIO' })
  @IsOptional()
  @Matches(/^[A-Za-z]{3}$/)
  moneda?: string;

  @ApiProperty({ required: false, description: 'Segmento al que aplica la lista', example: 'mayorista' })
  @IsOptional()
  @Matches(/^[a-z][a-z0-9_-]{0,39}$/)
  segmento_codigo?: string;

  @ApiProperty({ required: false, default: false })
  @IsOptional()
  @IsBoolean()
  predeterminada?: boolean;
}

export class ActualizarListaPrecioRequestDto extends PartialType(CrearListaPrecioRequestDto) {
  @ApiProperty({ required: false })
  @IsOptional()
  @IsBoolean()
  activo?: boolean;
}

export class CrearPrecioListaRequestDto {
  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  item_id: string;

  @ApiProperty({ required: false, minimum: 1, default: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  desde_cantidad?: number;

  @ApiProperty({ required: false, nullable: true, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  hasta_cantidad?: number | null;

  @ApiProperty({ minimum: 0 })
  @IsInt()
  @Min(0)
  precio_cents: number;
}

export class CrearEscalaRequestDto {
  @ApiProperty({ required: false, minimum: 1, default: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  desde_cantidad?: number;

  @ApiProperty({ required: false, nullable: true, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  hasta_cantidad?: number | null;

  @ApiProperty({ minimum: 0 })
  @IsInt()
  @Min(0)
  precio_cents: number;
}

export class ActualizarPrecioRequestDto {
  @ApiProperty({ required: false, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  desde_cantidad?: number;

  @ApiProperty({ required: false, nullable: true, minimum: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  hasta_cantidad?: number | null;

  @ApiProperty({ required: false, minimum: 0 })
  @IsOptional()
  @IsInt()
  @Min(0)
  precio_cents?: number;
}

export class ListaPrecioResponseDto implements ListaPrecio {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() descripcion: string;
  @ApiProperty() moneda: string;
  @ApiProperty() segmento_codigo: string;
  @ApiProperty() predeterminada: boolean;
  @ApiProperty() activo: boolean;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class PrecioItemResponseDto implements PrecioItem {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid', nullable: true }) lista_id: string | null;
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty() desde_cantidad: number;
  @ApiProperty({ nullable: true }) hasta_cantidad: number | null;
  @ApiProperty() precio_cents: number;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class ListaPrecioResumenResponseDto {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() segmento_codigo: string;
}

export class PrecioResueltoResponseDto implements PrecioResuelto {
  @ApiProperty({ format: 'uuid' }) item_id: string;
  @ApiProperty() cantidad: number;
  @ApiProperty({ type: () => ListaPrecioResumenResponseDto, nullable: true })
  lista: PrecioResuelto['lista'];
  @ApiProperty({ enum: ['lista', 'escala', 'base'] }) origen: PrecioResuelto['origen'];
  @ApiProperty({ type: () => PrecioItemResponseDto, nullable: true })
  escala: PrecioItem | null;
  @ApiProperty() precio_unitario_cents: number;
  @ApiProperty() precio_total_cents: number;
}

@ApiTags('Catálogo · Precios')
@ApiBearerAuth()
@Controller('api/v2/catalog')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.CATALOGO_ITEMS)
export class PreciosController {
  constructor(private readonly precios: PreciosService) {}

  // ── listas ─────────────────────────────────────────────────────────────

  @Get('listas-precio')
  @ApiOperation({ summary: 'Listar listas de precio de la empresa' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [ListaPrecioResponseDto], description: 'Listas de precio' })
  listarListas(
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.listarListas(this.empresaDe(usuario, empresaId));
  }

  @Post('listas-precio')
  @ApiOperation({ summary: 'Crear una lista de precio o segmento' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: ListaPrecioResponseDto, description: 'Lista creada' })
  crearLista(
    @Body() dto: CrearListaPrecioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.crearLista(this.empresaDe(usuario, empresaId), dto);
  }

  @Patch('listas-precio/:listaId')
  @ApiOperation({ summary: 'Actualizar una lista de precio' })
  @ApiParam({ name: 'listaId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: ListaPrecioResponseDto, description: 'Lista actualizada' })
  actualizarLista(
    @Param('listaId') listaId: string,
    @Body() dto: ActualizarListaPrecioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.actualizarLista(this.empresaDe(usuario, empresaId), listaId, dto);
  }

  // ── precios por lista ──────────────────────────────────────────────────

  @Get('listas-precio/:listaId/precios')
  @ApiOperation({ summary: 'Listar precios y escalas de una lista' })
  @ApiParam({ name: 'listaId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [PrecioItemResponseDto], description: 'Precios de la lista' })
  listarPrecios(
    @Param('listaId') listaId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.listarPrecios(this.empresaDe(usuario, empresaId), listaId);
  }

  @Post('listas-precio/:listaId/precios')
  @ApiOperation({ summary: 'Agregar un precio o rango de cantidad a una lista' })
  @ApiParam({ name: 'listaId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: PrecioItemResponseDto, description: 'Precio creado' })
  crearPrecio(
    @Param('listaId') listaId: string,
    @Body() dto: CrearPrecioListaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.crearPrecio(this.empresaDe(usuario, empresaId), listaId, dto);
  }

  @Patch('listas-precio/:listaId/precios/:precioId')
  @ApiOperation({ summary: 'Actualizar un precio de la lista' })
  @ApiParam({ name: 'listaId', format: 'uuid' })
  @ApiParam({ name: 'precioId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: PrecioItemResponseDto, description: 'Precio actualizado' })
  actualizarPrecio(
    @Param('listaId') listaId: string,
    @Param('precioId') precioId: string,
    @Body() dto: ActualizarPrecioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.actualizarPrecio(
      this.empresaDe(usuario, empresaId),
      listaId,
      precioId,
      dto,
    );
  }

  @Delete('listas-precio/:listaId/precios/:precioId')
  @ApiOperation({ summary: 'Eliminar un precio de la lista' })
  @ApiParam({ name: 'listaId', format: 'uuid' })
  @ApiParam({ name: 'precioId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ description: 'Precio eliminado' })
  eliminarPrecio(
    @Param('listaId') listaId: string,
    @Param('precioId') precioId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.eliminarPrecio(this.empresaDe(usuario, empresaId), listaId, precioId);
  }

  // ── escalas base del item ──────────────────────────────────────────────

  @Get('items/:id/escalas')
  @ApiOperation({ summary: 'Listar escalas base de un item (tipo_precio = escala)' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [PrecioItemResponseDto], description: 'Escalas del item' })
  listarEscalas(
    @Param('id') itemId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.listarEscalas(this.empresaDe(usuario, empresaId), itemId);
  }

  @Post('items/:id/escalas')
  @ApiOperation({ summary: 'Agregar una escala de cantidad al item' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: PrecioItemResponseDto, description: 'Escala creada' })
  crearEscala(
    @Param('id') itemId: string,
    @Body() dto: CrearEscalaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.crearPrecio(this.empresaDe(usuario, empresaId), null, {
      ...dto,
      item_id: itemId,
    });
  }

  @Patch('items/:id/escalas/:escalaId')
  @ApiOperation({ summary: 'Actualizar una escala del item' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiParam({ name: 'escalaId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: PrecioItemResponseDto, description: 'Escala actualizada' })
  actualizarEscala(
    @Param('id') itemId: string,
    @Param('escalaId') escalaId: string,
    @Body() dto: ActualizarPrecioRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.actualizarEscala(
      this.empresaDe(usuario, empresaId),
      itemId,
      escalaId,
      dto,
    );
  }

  @Delete('items/:id/escalas/:escalaId')
  @ApiOperation({ summary: 'Eliminar una escala del item' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiParam({ name: 'escalaId', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ description: 'Escala eliminada' })
  eliminarEscala(
    @Param('id') itemId: string,
    @Param('escalaId') escalaId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.eliminarEscala(this.empresaDe(usuario, empresaId), itemId, escalaId);
  }

  // ── resolucion ──────────────────────────────────────────────────────────

  @Get('items/:id/precio')
  @ApiOperation({ summary: 'Resolver el precio unitario por cantidad, lista o segmento' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'cantidad', required: false, example: 1 })
  @ApiQuery({ name: 'lista', required: false, example: 'mayorista' })
  @ApiQuery({ name: 'segmento', required: false, example: 'mayorista' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: PrecioResueltoResponseDto, description: 'Precio resuelto' })
  resolver(
    @Param('id') itemId: string,
    @Query('cantidad') cantidad: string | undefined,
    @Query('lista') lista: string | undefined,
    @Query('segmento') segmento: string | undefined,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.precios.resolver(this.empresaDe(usuario, empresaId), itemId, cantidad ?? 1, {
      lista_codigo: lista,
      segmento_codigo: segmento,
    });
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
