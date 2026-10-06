import { Controller, Get, Post, Param, Body, Query } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiProperty, ApiTags } from '@nestjs/swagger';
import { Type } from 'class-transformer';
import {
  ArrayMinSize,
  IsIn,
  IsInt,
  IsOptional,
  IsString,
  IsDateString,
  Min,
  MinLength,
  ValidateNested,
} from 'class-validator';
import { Roles, ROLES, UsuarioActual, UsuarioContexto, NotFoundError } from '@core/shared';
import { ContabilidadService, Asiento, Cuenta, ParametrosAsiento } from './contabilidad.service';

export class DetalleAsientoRequestDto {
  @ApiProperty({ description: 'Código de cuenta contable' })
  @IsString()
  cuenta_codigo: string;

  @ApiProperty({ description: 'Débito en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  debe_cents: number;

  @ApiProperty({ description: 'Crédito en centavos', minimum: 0 })
  @IsInt()
  @Min(0)
  haber_cents: number;

  @ApiProperty({ description: 'Concepto del detalle', required: false })
  @IsOptional()
  @IsString()
  concepto?: string;

  @ApiProperty({ description: 'Orden del detalle', required: false })
  @IsOptional()
  @IsInt()
  orden?: number;
}

export class CrearAsientoRequestDto {
  @ApiProperty({ description: 'Concepto del asiento', minLength: 3 })
  @IsString()
  @MinLength(3)
  concepto: string;

  @ApiProperty({ description: 'Fecha (ISO 8601)', required: false })
  @IsOptional()
  @IsDateString()
  fecha?: string;

  @ApiProperty({ description: 'Tipo de asiento', required: false, enum: ['INGRESO', 'EGRESO', 'AJUSTE', 'CIERRE', 'APERTURA', 'MANUAL'] })
  @IsOptional()
  @IsIn(['INGRESO', 'EGRESO', 'AJUSTE', 'CIERRE', 'APERTURA', 'MANUAL'])
  tipo?: string;

  @ApiProperty({ description: 'Tipo de referencia', required: false })
  @IsOptional()
  @IsString()
  referencia_tipo?: string;

  @ApiProperty({ description: 'Id de referencia', required: false })
  @IsOptional()
  @IsString()
  referencia_id?: string;

  @ApiProperty({ description: 'Detalles del asiento (mínimo 2)', type: [DetalleAsientoRequestDto] })
  @ArrayMinSize(2)
  @ValidateNested({ each: true })
  @Type(() => DetalleAsientoRequestDto)
  detalles: DetalleAsientoRequestDto[];
}

export class CrearCuentaRequestDto {
  @ApiProperty({ description: 'Código de la cuenta' })
  @IsString()
  codigo: string;

  @ApiProperty({ description: 'Nombre de la cuenta', minLength: 3 })
  @IsString()
  @MinLength(3)
  nombre: string;

  @ApiProperty({ description: 'Tipo de cuenta', enum: ['ACTIVO', 'PASIVO', 'CAPITAL', 'INGRESO', 'COSTO', 'GASTO'] })
  @IsIn(['ACTIVO', 'PASIVO', 'CAPITAL', 'INGRESO', 'COSTO', 'GASTO'])
  tipo: Cuenta['tipo'];

  @ApiProperty({ description: 'Naturaleza de la cuenta', enum: ['DEUDORA', 'ACREEDORA'] })
  @IsIn(['DEUDORA', 'ACREEDORA'])
  naturaleza: Cuenta['naturaleza'];

  @ApiProperty({ description: 'Nivel jerárquico', required: false })
  @IsOptional()
  @IsInt()
  nivel?: number;
}

export class EstadoCuentaRequestDto {
  @ApiProperty({ description: 'Estado de la cuenta', enum: ['activa', 'inactiva'] })
  @IsIn(['activa', 'inactiva'])
  estado: string;
}

/**
 * GET/POST /api/v1/finanzas/{cuentas,asientos,libro-mayor} — contabilidad
 * por partida doble: plan de cuentas, libro diario e historico por cuenta.
 */
@ApiTags('Finanzas · Contabilidad')
@Controller('api/v1/finanzas')
@Roles(ROLES.ADMIN)
export class ContabilidadController {
  constructor(private readonly contabilidad: ContabilidadService) {}

  @Get('cuentas')
  @ApiOperation({ summary: 'Plan de cuentas' })
  async cuentas(): Promise<Cuenta[]> {
    return this.contabilidad.planDeCuentas();
  }

  @Post('cuentas')
  @ApiOperation({ summary: 'Crear una cuenta contable' })
  async crearCuenta(@Body() dto: CrearCuentaRequestDto): Promise<Cuenta> {
    return this.contabilidad.crearCuenta(dto);
  }

  @Post('cuentas/:codigo/estado')
  @ApiOperation({ summary: 'Cambiar el estado de una cuenta' })
  @ApiParam({ name: 'codigo', description: 'Código de la cuenta' })
  async estadoCuenta(@Param('codigo') codigo: string, @Body() dto: EstadoCuentaRequestDto): Promise<Cuenta> {
    const cuenta = await this.contabilidad.estadoCuenta(codigo, dto.estado as 'activa' | 'inactiva');
    if (!cuenta) throw new NotFoundError('Cuenta', codigo);
    return cuenta;
  }

  /** Libro diario: GET /api/v1/finanzas/asientos?desde=&hasta= */
  @Get('asientos')
  @ApiOperation({ summary: 'Libro diario (asientos)' })
  async libroDiario(
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
    @Query('limite') limite?: string,
  ): Promise<Asiento[]> {
    return this.contabilidad.libroDiario(desde, hasta, limite ? Number(limite) : 200);
  }

  /** Asiento manual del administrador (bitacora append-only). */
  @Post('asientos')
  @ApiOperation({ summary: 'Registrar un asiento manual' })
  async registrar(
    @Body() dto: CrearAsientoRequestDto,
    @UsuarioActual() usuario: UsuarioContexto,
  ): Promise<Asiento> {
    return this.contabilidad.registrar({
      concepto: dto.concepto,
      tipo: (dto.tipo as ParametrosAsiento['tipo']) ?? 'MANUAL',
      fecha: dto.fecha,
      referencia_tipo: dto.referencia_tipo,
      referencia_id: dto.referencia_id,
      creado_por: usuario?.user_id,
      detalles: dto.detalles,
    });
  }

  @Post('asientos/:id/anular')
  @ApiOperation({ summary: 'Anular un asiento' })
  @ApiParam({ name: 'id', description: 'Id del asiento' })
  async anular(@Param('id') id: string): Promise<Asiento> {
    const asiento = await this.contabilidad.anular(id);
    if (!asiento) throw new NotFoundError('Asiento', id);
    return asiento;
  }

  /** Libro mayor por cuenta: GET /api/v1/finanzas/libro-mayor/:cuenta?desde=&hasta= */
  @Get('libro-mayor/:cuenta')
  @ApiOperation({ summary: 'Libro mayor por cuenta' })
  @ApiParam({ name: 'cuenta', description: 'Código de la cuenta' })
  async libroMayor(
    @Param('cuenta') cuenta: string,
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
  ) {
    return this.contabilidad.libroMayor(cuenta, desde, hasta);
  }

  /** Libro de ventas (DGI): GET /api/v1/finanzas/libro-ventas?desde=&hasta= */
  @Get('libro-ventas')
  @ApiOperation({ summary: 'Libro de ventas (DGI)' })
  async libroVentas(@Query('desde') desde?: string, @Query('hasta') hasta?: string) {
    return this.contabilidad.libroVentas(desde, hasta);
  }

  /** Libro de compras (DGI): GET /api/v1/finanzas/libro-compras?desde=&hasta= */
  @Get('libro-compras')
  @ApiOperation({ summary: 'Libro de compras (DGI)' })
  async libroCompras(@Query('desde') desde?: string, @Query('hasta') hasta?: string) {
    return this.contabilidad.libroCompras(desde, hasta);
  }
}
