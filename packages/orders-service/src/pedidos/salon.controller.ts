import { Body, Controller, Get, Param, Patch, Post, Query } from '@nestjs/common';
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
import { Type } from 'class-transformer';
import {
  IsArray,
  IsIn,
  IsInt,
  IsOptional,
  IsString,
  IsUUID,
  Matches,
  Max,
  Min,
  ValidateNested,
} from 'class-validator';
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
  Comanda,
  ComandaItem,
  CrearComandaInput,
  CrearMesaInput,
  ESTADOS_COMANDA,
  ESTADOS_MESA,
  EstadoComanda,
  EstadoMesa,
  Mesa,
  SalonService,
} from './salon.service';

export class CrearMesaRequestDto {
  @ApiProperty({ example: 'M-01' })
  @Matches(/^[A-Za-z0-9_-]{1,20}$/)
  codigo: string;

  @ApiProperty({ example: 'Mesa 1' })
  @IsString()
  nombre: string;

  @ApiProperty({ required: false, minimum: 1, maximum: 999, default: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(999)
  capacidad?: number;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  zona?: string;

  @ApiProperty({ required: false, enum: ESTADOS_MESA, default: 'libre' })
  @IsOptional()
  @IsIn(ESTADOS_MESA)
  estado?: EstadoMesa;
}

export class ActualizarMesaRequestDto extends PartialType(CrearMesaRequestDto) {}

export class ComandaItemRequestDto implements ComandaItem {
  @ApiProperty() @IsString() sku: string;
  @ApiProperty() @IsString() nombre: string;
  @ApiProperty({ minimum: 1 }) @IsInt() @Min(1) cantidad: number;
  @ApiProperty({ required: false }) @IsOptional() @IsString() notas?: string;
}

export class CrearComandaRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  order_id?: string;

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  mesa_id?: string;

  @ApiProperty({ example: 'cocina' })
  @IsString()
  estacion: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  notas?: string;

  @ApiProperty({ required: false, type: [ComandaItemRequestDto] })
  @IsOptional()
  @IsArray()
  @ValidateNested({ each: true })
  @Type(() => ComandaItemRequestDto)
  items?: ComandaItemRequestDto[];
}

export class ActualizarComandaRequestDto {
  @ApiProperty({ required: false, enum: ESTADOS_COMANDA })
  @IsOptional()
  @IsIn(ESTADOS_COMANDA)
  estado?: EstadoComanda;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  notas?: string;
}

export class MesaResponseDto implements Mesa {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty() codigo: string;
  @ApiProperty() nombre: string;
  @ApiProperty() capacidad: number;
  @ApiProperty() zona: string;
  @ApiProperty({ enum: ESTADOS_MESA }) estado: EstadoMesa;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class ComandaResponseDto implements Comanda {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid', nullable: true }) order_id: string | null;
  @ApiProperty({ format: 'uuid', nullable: true }) mesa_id: string | null;
  @ApiProperty() estacion: string;
  @ApiProperty({ enum: ESTADOS_COMANDA }) estado: EstadoComanda;
  @ApiProperty() notas: string;
  @ApiProperty({ type: [ComandaItemRequestDto] }) items: ComandaItem[];
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
  @ApiProperty({ format: 'date-time', nullable: true }) listo_en: string | null;
  @ApiProperty({ format: 'date-time', nullable: true }) entregado_en: string | null;
}

@ApiTags('Salón · Mesas y comandas')
@ApiBearerAuth()
@Controller('api/v2')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.MESAS)
export class SalonController {
  constructor(private readonly salon: SalonService) {}

  @Get('mesas')
  @ApiOperation({ summary: 'Listar mesas de la empresa' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiQuery({ name: 'zona', required: false })
  @ApiQuery({ name: 'estado', required: false, enum: ESTADOS_MESA })
  @ApiOkResponse({ type: [MesaResponseDto], description: 'Mesas' })
  listarMesas(
    @Query('empresa_id') empresaId: string | undefined,
    @Query('zona') zona: string | undefined,
    @Query('estado') estado: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.salon.listarMesas(this.empresaDe(usuario, empresaId), { zona, estado });
  }

  @Post('mesas')
  @ApiOperation({ summary: 'Crear una mesa' })
  @ApiCreatedResponse({ type: MesaResponseDto, description: 'Mesa creada' })
  crearMesa(
    @Body() dto: CrearMesaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.salon.crearMesa(this.empresaDe(usuario, empresaId), dto as CrearMesaInput);
  }

  @Patch('mesas/:id')
  @ApiOperation({ summary: 'Actualizar una mesa o su estado' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: MesaResponseDto, description: 'Mesa actualizada' })
  actualizarMesa(
    @Param('id') mesaId: string,
    @Body() dto: ActualizarMesaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.salon.actualizarMesa(this.empresaDe(usuario, empresaId), mesaId, dto);
  }

  @Get('comandas')
  @ApiOperation({ summary: 'Listar comandas de preparacion' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiQuery({ name: 'estado', required: false, enum: ESTADOS_COMANDA })
  @ApiQuery({ name: 'estacion', required: false })
  @ApiQuery({ name: 'mesa_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: [ComandaResponseDto], description: 'Comandas' })
  listarComandas(
    @Query('empresa_id') empresaId: string | undefined,
    @Query('estado') estado: string | undefined,
    @Query('estacion') estacion: string | undefined,
    @Query('mesa_id') mesaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.salon.listarComandas(this.empresaDe(usuario, empresaId), {
      estado,
      estacion,
      mesa_id: mesaId,
    });
  }

  @Post('comandas')
  @ApiOperation({ summary: 'Crear una comanda desde una orden o items libres' })
  @ApiCreatedResponse({ type: ComandaResponseDto, description: 'Comanda creada' })
  crearComanda(
    @Body() dto: CrearComandaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.salon.crearComanda(
      this.empresaDe(usuario, empresaId),
      dto as CrearComandaInput,
    );
  }

  @Patch('comandas/:id')
  @ApiOperation({ summary: 'Avanzar el estado de una comanda' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: ComandaResponseDto, description: 'Comanda actualizada' })
  actualizarComanda(
    @Param('id') comandaId: string,
    @Body() dto: ActualizarComandaRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.salon.actualizarComanda(
      this.empresaDe(usuario, empresaId),
      comandaId,
      dto,
    );
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
