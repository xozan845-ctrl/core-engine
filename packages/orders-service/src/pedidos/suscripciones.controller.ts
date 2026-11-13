import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
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
import {
  IsIn,
  IsInt,
  IsObject,
  IsOptional,
  IsUUID,
  Matches,
  Max,
  Min,
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
  CrearSuscripcionInput,
  FULFILLMENTS_SUSCRIPCION,
  INTERVALOS,
  IntervaloSuscripcion,
  Suscripcion,
  SuscripcionEjecucion,
  SuscripcionesService,
} from './suscripciones.service';

const FECHA = /^\d{4}-\d{2}-\d{2}$/;

export class CrearSuscripcionRequestDto {
  @ApiProperty({ format: 'uuid' })
  @IsUUID()
  oferta_id: string;

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  item_id?: string;

  @ApiProperty({ required: false, minimum: 1, maximum: 99, default: 1 })
  @IsOptional()
  @IsInt()
  @Min(1)
  @Max(99)
  cantidad?: number;

  @ApiProperty({ required: false, format: 'uuid', description: 'Solo admin/empresario' })
  @IsOptional()
  @IsUUID()
  comprador_id?: string;

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  cliente_parte_id?: string;

  @ApiProperty({ required: false, enum: ['simulado', 'credito'], default: 'simulado' })
  @IsOptional()
  @IsIn(['simulado', 'credito'])
  metodo_pago?: 'simulado' | 'credito';

  @ApiProperty({ required: false, enum: FULFILLMENTS_SUSCRIPCION, default: 'none' })
  @IsOptional()
  @IsIn(FULFILLMENTS_SUSCRIPCION)
  fulfillment?: string;

  @ApiProperty({ enum: INTERVALOS })
  @IsIn(INTERVALOS)
  intervalo: IntervaloSuscripcion;

  @ApiProperty({ required: false, format: 'date' })
  @IsOptional()
  @Matches(FECHA)
  proxima_ejecucion?: string;

  @ApiProperty({ required: false, format: 'date' })
  @IsOptional()
  @Matches(FECHA)
  fin_en?: string;

  @ApiProperty({ required: false, type: Object, additionalProperties: true })
  @IsOptional()
  @IsObject()
  metadata?: Record<string, unknown>;
}

export class SuscripcionResponseDto implements Suscripcion {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid' }) oferta_id: string;
  @ApiProperty({ format: 'uuid', nullable: true }) item_id: string | null;
  @ApiProperty() cantidad: number;
  @ApiProperty({ format: 'uuid' }) comprador_id: string;
  @ApiProperty({ format: 'uuid', nullable: true }) cliente_parte_id: string | null;
  @ApiProperty({ enum: ['compra'] }) tipo_orden: 'compra';
  @ApiProperty({ enum: ['simulado', 'credito'] }) metodo_pago: 'simulado' | 'credito';
  @ApiProperty() fulfillment: string;
  @ApiProperty({ enum: INTERVALOS }) intervalo: IntervaloSuscripcion;
  @ApiProperty({ enum: ['activa', 'pausada', 'cancelada'] }) estado: Suscripcion['estado'];
  @ApiProperty({ format: 'date' }) proxima_ejecucion: string;
  @ApiProperty({ format: 'date', nullable: true }) ultima_ejecucion: string | null;
  @ApiProperty({ format: 'date', nullable: true }) fin_en: string | null;
  @ApiProperty({ type: Object, additionalProperties: true }) metadata: Record<string, unknown>;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
}

export class SuscripcionEjecucionResponseDto implements SuscripcionEjecucion {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) suscripcion_id: string;
  @ApiProperty({ format: 'uuid', nullable: true }) order_id: string | null;
  @ApiProperty({ enum: ['creada', 'error'] }) estado: 'creada' | 'error';
  @ApiProperty() error: string;
  @ApiProperty({ format: 'date-time' }) ejecutada_en: string;
}

export class RenovarSuscripcionResponseDto {
  @ApiProperty({ type: () => SuscripcionResponseDto }) suscripcion: Suscripcion;
  @ApiProperty({ format: 'uuid' }) order_id: string;
}

@ApiTags('Suscripciones')
@ApiBearerAuth()
@Controller('api/v2/suscripciones')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.SUSCRIPCIONES)
export class SuscripcionesController {
  constructor(private readonly suscripciones: SuscripcionesService) {}

  @Get()
  @ApiOperation({ summary: 'Listar suscripciones de la empresa' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiQuery({ name: 'estado', required: false })
  @ApiQuery({ name: 'comprador_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: [SuscripcionResponseDto], description: 'Suscripciones' })
  listar(
    @Query('empresa_id') empresaId: string | undefined,
    @Query('estado') estado: string | undefined,
    @Query('comprador_id') compradorId: string | undefined,
    @Query('limite') limite: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.listar(this.empresaDe(usuario, empresaId), {
      estado,
      comprador_id: compradorId,
      limite,
    });
  }

  @Post()
  @ApiOperation({ summary: 'Crear una suscripcion a una oferta' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: SuscripcionResponseDto, description: 'Suscripcion creada' })
  crear(
    @Body() dto: CrearSuscripcionRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.crear(
      this.empresaDe(usuario, empresaId),
      usuario.user_id,
      dto as CrearSuscripcionInput,
    );
  }

  @Get(':id')
  @ApiOperation({ summary: 'Obtener una suscripcion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: SuscripcionResponseDto, description: 'Suscripcion encontrada' })
  obtener(
    @Param('id') suscripcionId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.obtener(this.empresaDe(usuario, empresaId), suscripcionId);
  }

  @Post(':id/pausar')
  @ApiOperation({ summary: 'Pausar una suscripcion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: SuscripcionResponseDto, description: 'Suscripcion pausada' })
  pausar(
    @Param('id') suscripcionId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.pausar(this.empresaDe(usuario, empresaId), suscripcionId);
  }

  @Post(':id/reanudar')
  @ApiOperation({ summary: 'Reanudar una suscripcion pausada' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: SuscripcionResponseDto, description: 'Suscripcion reanudada' })
  reanudar(
    @Param('id') suscripcionId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.reanudar(this.empresaDe(usuario, empresaId), suscripcionId);
  }

  @Post(':id/cancelar')
  @ApiOperation({ summary: 'Cancelar una suscripcion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: SuscripcionResponseDto, description: 'Suscripcion cancelada' })
  cancelar(
    @Param('id') suscripcionId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.cancelar(this.empresaDe(usuario, empresaId), suscripcionId);
  }

  @Post(':id/renovar')
  @ApiOperation({ summary: 'Renovar ahora y crear la orden de la suscripcion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiOkResponse({ type: RenovarSuscripcionResponseDto, description: 'Suscripcion renovada' })
  renovar(
    @Param('id') suscripcionId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.renovar(this.empresaDe(usuario, empresaId), suscripcionId, true);
  }

  @Get(':id/ejecuciones')
  @ApiOperation({ summary: 'Listar las renovaciones de una suscripcion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [SuscripcionEjecucionResponseDto], description: 'Ejecuciones' })
  ejecuciones(
    @Param('id') suscripcionId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.suscripciones.listarEjecuciones(
      this.empresaDe(usuario, empresaId),
      suscripcionId,
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
