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
import {
  IsIn,
  IsObject,
  IsOptional,
  IsString,
  IsUUID,
  Matches,
  MaxLength,
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
  Contrato,
  ContratoEvento,
  ContratosService,
  EstadoContrato,
  TIPOS_CONTRATO,
  TipoContrato,
} from './contratos.service';

const MONTO = /^\d+(\.\d{1,2})?$/;
const FECHA = /^\d{4}-\d{2}-\d{2}$/;

export class CrearContratoRequestDto {
  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  order_id?: string;

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  parte_id?: string;

  @ApiProperty({ required: false, format: 'uuid' })
  @IsOptional()
  @IsUUID()
  item_id?: string;

  @ApiProperty({ required: false, enum: TIPOS_CONTRATO, default: 'otro' })
  @IsOptional()
  @IsIn(TIPOS_CONTRATO)
  tipo?: TipoContrato;

  @ApiProperty({ example: 'Alquiler de equipo' })
  @IsString()
  titulo: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ required: false, format: 'date' })
  @IsOptional()
  @Matches(FECHA)
  inicia_en?: string;

  @ApiProperty({ required: false, format: 'date' })
  @IsOptional()
  @Matches(FECHA)
  termina_en?: string;

  @ApiProperty({ required: false, example: '500.00' })
  @IsOptional()
  @Matches(MONTO)
  deposito?: string;

  @ApiProperty({ required: false, example: '200.00' })
  @IsOptional()
  @Matches(MONTO)
  garantia?: string;

  @ApiProperty({ required: false, default: 'NIO' })
  @IsOptional()
  @Matches(/^[A-Za-z]{3}$/)
  moneda?: string;

  @ApiProperty({ required: false, type: Object, additionalProperties: true })
  @IsOptional()
  @IsObject()
  metadata?: Record<string, unknown>;
}

export class ActualizarContratoRequestDto extends PartialType(CrearContratoRequestDto) {
  @ApiProperty({ required: false, enum: ['activo', 'cancelado'] })
  @IsOptional()
  @IsIn(['activo', 'cancelado'])
  estado?: 'activo' | 'cancelado';
}

export class FinalizarContratoRequestDto {
  @ApiProperty({ required: false, example: '50.00' })
  @IsOptional()
  @Matches(MONTO)
  retenido?: string;

  @ApiProperty({ required: false })
  @IsOptional()
  @IsString()
  @MaxLength(500)
  motivo?: string;
}

export class ContratoResponseDto implements Contrato {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) empresa_id: string;
  @ApiProperty({ format: 'uuid', nullable: true }) order_id: string | null;
  @ApiProperty({ format: 'uuid', nullable: true }) parte_id: string | null;
  @ApiProperty({ format: 'uuid', nullable: true }) item_id: string | null;
  @ApiProperty({ enum: TIPOS_CONTRATO }) tipo: TipoContrato;
  @ApiProperty() titulo: string;
  @ApiProperty() descripcion: string;
  @ApiProperty({ enum: ['borrador', 'activo', 'finalizado', 'cancelado'] }) estado: EstadoContrato;
  @ApiProperty({ format: 'date', nullable: true }) inicia_en: string | null;
  @ApiProperty({ format: 'date', nullable: true }) termina_en: string | null;
  @ApiProperty() deposito: string;
  @ApiProperty() garantia: string;
  @ApiProperty() retenido: string;
  @ApiProperty() moneda: string;
  @ApiProperty({ type: Object, additionalProperties: true }) metadata: Record<string, unknown>;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
  @ApiProperty({ format: 'date-time' }) actualizado_en: string;
  @ApiProperty({ format: 'date-time', nullable: true }) finalizado_en: string | null;
}

export class ContratoEventoResponseDto implements ContratoEvento {
  @ApiProperty({ format: 'uuid' }) id: string;
  @ApiProperty({ format: 'uuid' }) contrato_id: string;
  @ApiProperty() tipo: string;
  @ApiProperty() monto: string;
  @ApiProperty() motivo: string;
  @ApiProperty({ format: 'uuid' }) creado_por: string;
  @ApiProperty({ format: 'date-time' }) creado_en: string;
}

@ApiTags('Contratos')
@ApiBearerAuth()
@Controller('api/v2/contratos')
@Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN, ROLES.EMPRESARIO)
@RequiereCapacidades(CAPACIDADES.CONTRATOS)
export class ContratosController {
  constructor(private readonly contratos: ContratosService) {}

  @Get()
  @ApiOperation({ summary: 'Listar contratos de la empresa' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiQuery({ name: 'estado', required: false })
  @ApiQuery({ name: 'order_id', required: false, format: 'uuid' })
  @ApiQuery({ name: 'parte_id', required: false, format: 'uuid' })
  @ApiOkResponse({ type: [ContratoResponseDto], description: 'Contratos' })
  listar(
    @Query('empresa_id') empresaId: string | undefined,
    @Query('estado') estado?: string,
    @Query('order_id') orderId?: string,
    @Query('parte_id') parteId?: string,
    @Query('pagina') pagina?: string,
    @Query('limite') limite?: string,
    @UsuarioActual() usuario?: UsuarioContexto,
  ) {
    return this.contratos.listar(this.empresaDe(usuario as UsuarioContexto, empresaId), {
      estado,
      order_id: orderId,
      parte_id: parteId,
      pagina,
      limite,
    });
  }

  @Post()
  @ApiOperation({ summary: 'Crear un contrato con deposito o garantia' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiCreatedResponse({ type: ContratoResponseDto, description: 'Contrato creado' })
  crear(
    @Body() dto: CrearContratoRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.contratos.crear(this.empresaDe(usuario, empresaId), usuario.user_id, dto);
  }

  @Get(':id')
  @ApiOperation({ summary: 'Obtener un contrato' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: ContratoResponseDto, description: 'Contrato encontrado' })
  obtener(
    @Param('id') contratoId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.contratos.obtener(this.empresaDe(usuario, empresaId), contratoId);
  }

  @Patch(':id')
  @ApiOperation({ summary: 'Actualizar o activar/cancelar un contrato' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: ContratoResponseDto, description: 'Contrato actualizado' })
  actualizar(
    @Param('id') contratoId: string,
    @Body() dto: ActualizarContratoRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.contratos.actualizar(this.empresaDe(usuario, empresaId), contratoId, dto);
  }

  @Post(':id/finalizar')
  @ApiOperation({ summary: 'Finalizar el contrato y registrar la retencion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: ContratoResponseDto, description: 'Contrato finalizado' })
  finalizar(
    @Param('id') contratoId: string,
    @Body() dto: FinalizarContratoRequestDto,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.contratos.finalizar(
      this.empresaDe(usuario, empresaId),
      contratoId,
      usuario.user_id,
      dto,
    );
  }

  @Get(':id/movimientos')
  @ApiOperation({ summary: 'Listar los movimientos de deposito y retencion' })
  @ApiParam({ name: 'id', format: 'uuid' })
  @ApiQuery({ name: 'empresa_id', required: false, description: 'Solo admin global' })
  @ApiOkResponse({ type: [ContratoEventoResponseDto], description: 'Movimientos del contrato' })
  movimientos(
    @Param('id') contratoId: string,
    @Query('empresa_id') empresaId: string | undefined,
    @UsuarioActual() usuario: UsuarioContexto,
  ) {
    return this.contratos.listarEventos(this.empresaDe(usuario, empresaId), contratoId);
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
