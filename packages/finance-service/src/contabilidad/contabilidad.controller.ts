import { Controller, Get, Post, Param, Body, Query } from '@nestjs/common';
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

export class DetalleAsientoDto {
  @IsString()
  cuenta_codigo: string;

  @IsInt()
  @Min(0)
  debe_cents: number;

  @IsInt()
  @Min(0)
  haber_cents: number;

  @IsOptional()
  @IsString()
  concepto?: string;

  @IsOptional()
  @IsInt()
  orden?: number;
}

export class CrearAsientoDto {
  @IsString()
  @MinLength(3)
  concepto: string;

  @IsOptional()
  @IsDateString()
  fecha?: string;

  @IsOptional()
  @IsIn(['INGRESO', 'EGRESO', 'AJUSTE', 'CIERRE', 'APERTURA', 'MANUAL'])
  tipo?: string;

  @IsOptional()
  @IsString()
  referencia_tipo?: string;

  @IsOptional()
  @IsString()
  referencia_id?: string;

  @ArrayMinSize(2)
  @ValidateNested({ each: true })
  @Type(() => DetalleAsientoDto)
  detalles: DetalleAsientoDto[];
}

export class CrearCuentaDto {
  @IsString()
  codigo: string;

  @IsString()
  @MinLength(3)
  nombre: string;

  @IsIn(['ACTIVO', 'PASIVO', 'CAPITAL', 'INGRESO', 'COSTO', 'GASTO'])
  tipo: Cuenta['tipo'];

  @IsIn(['DEUDORA', 'ACREEDORA'])
  naturaleza: Cuenta['naturaleza'];

  @IsOptional()
  @IsInt()
  nivel?: number;
}

export class EstadoCuentaDto {
  @IsIn(['activa', 'inactiva'])
  estado: string;
}

/**
 * GET/POST /api/v1/finanzas/{cuentas,asientos,libro-mayor} — contabilidad
 * por partida doble: plan de cuentas, libro diario e historico por cuenta.
 */
@Controller('api/v1/finanzas')
@Roles(ROLES.ADMIN)
export class ContabilidadController {
  constructor(private readonly contabilidad: ContabilidadService) {}

  @Get('cuentas')
  async cuentas(): Promise<Cuenta[]> {
    return this.contabilidad.planDeCuentas();
  }

  @Post('cuentas')
  async crearCuenta(@Body() dto: CrearCuentaDto): Promise<Cuenta> {
    return this.contabilidad.crearCuenta(dto);
  }

  @Post('cuentas/:codigo/estado')
  async estadoCuenta(@Param('codigo') codigo: string, @Body() dto: EstadoCuentaDto): Promise<Cuenta> {
    const cuenta = await this.contabilidad.estadoCuenta(codigo, dto.estado as 'activa' | 'inactiva');
    if (!cuenta) throw new NotFoundError('Cuenta', codigo);
    return cuenta;
  }

  /** Libro diario: GET /api/v1/finanzas/asientos?desde=&hasta= */
  @Get('asientos')
  async libroDiario(
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
    @Query('limite') limite?: string,
  ): Promise<Asiento[]> {
    return this.contabilidad.libroDiario(desde, hasta, limite ? Number(limite) : 200);
  }

  /** Asiento manual del administrador (bitacora append-only). */
  @Post('asientos')
  async registrar(
    @Body() dto: CrearAsientoDto,
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
  async anular(@Param('id') id: string): Promise<Asiento> {
    const asiento = await this.contabilidad.anular(id);
    if (!asiento) throw new NotFoundError('Asiento', id);
    return asiento;
  }

  /** Libro mayor por cuenta: GET /api/v1/finanzas/libro-mayor/:cuenta?desde=&hasta= */
  @Get('libro-mayor/:cuenta')
  async libroMayor(
    @Param('cuenta') cuenta: string,
    @Query('desde') desde?: string,
    @Query('hasta') hasta?: string,
  ) {
    return this.contabilidad.libroMayor(cuenta, desde, hasta);
  }

  /** Libro de ventas (DGI): GET /api/v1/finanzas/libro-ventas?desde=&hasta= */
  @Get('libro-ventas')
  async libroVentas(@Query('desde') desde?: string, @Query('hasta') hasta?: string) {
    return this.contabilidad.libroVentas(desde, hasta);
  }

  /** Libro de compras (DGI): GET /api/v1/finanzas/libro-compras?desde=&hasta= */
  @Get('libro-compras')
  async libroCompras(@Query('desde') desde?: string, @Query('hasta') hasta?: string) {
    return this.contabilidad.libroCompras(desde, hasta);
  }
}