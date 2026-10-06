import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
import { ApiOperation, ApiParam, ApiProperty, ApiQuery, ApiTags } from '@nestjs/swagger';
import { IsBoolean, IsIn, IsInt, IsOptional, IsString, IsUUID, Min, MinLength } from 'class-validator';
import { Roles, ROLES, NotFoundError } from '@core/shared';
import {
  TributacionService,
  Jurisdiccion,
  RegimenFiscal,
  SujetoTributario,
  Declaracion,
} from './tributacion.service';

export class CrearJurisdiccionRequestDto {
  @ApiProperty({ description: 'Código de país (ISO)', minLength: 2 })
  @IsString()
  @MinLength(2)
  codigo_pais: string;

  @ApiProperty({ description: 'Nombre de la jurisdicción' })
  @IsString()
  nombre: string;

  @ApiProperty({ description: 'Moneda' })
  @IsString()
  moneda: string;

  @ApiProperty({ description: 'Símbolo de la moneda' })
  @IsString()
  simbolo_moneda: string;

  @ApiProperty({ description: 'Tasa de IVA por mil', minimum: 0 })
  @IsInt()
  @Min(0)
  tasa_iva_por_mil: number;

  @ApiProperty({ description: 'Tasa de IR por mil', minimum: 0 })
  @IsInt()
  @Min(0)
  tasa_ir_por_mil: number;

  @ApiProperty({ description: 'Periodicidad de declaración', required: false })
  @IsOptional()
  @IsString()
  periodicidad_declaracion?: string;

  @ApiProperty({ description: 'Leyes aplicables (clave→texto)', required: false })
  @IsOptional()
  leyes?: Record<string, string>;
}

export class CrearRegimenRequestDto {
  @ApiProperty({ description: 'Jurisdicción', minLength: 2 })
  @IsString()
  @MinLength(2)
  jurisdiccion: string;

  @ApiProperty({ description: 'Código del régimen' })
  @IsString()
  codigo: string;

  @ApiProperty({ description: 'Nombre del régimen' })
  @IsString()
  nombre: string;

  @ApiProperty({ description: 'Descripción', required: false })
  @IsOptional()
  @IsString()
  descripcion?: string;

  @ApiProperty({ description: 'Periodicidad', required: false })
  @IsOptional()
  @IsString()
  periodicidad?: string;

  @ApiProperty({ description: 'Condición de ingresos anuales (centavos)', required: false })
  @IsOptional()
  @IsInt()
  condicion_ingresos_anuales_cents?: number;
}

export class RegistrarSujetoRequestDto {
  @ApiProperty({ description: 'Id del sujeto (UUID)' })
  @IsUUID()
  id: string;

  @ApiProperty({ description: 'Jurisdicción', required: false })
  @IsOptional()
  @IsString()
  jurisdiccion?: string;

  @ApiProperty({ description: 'Id del régimen (UUID)', required: false })
  @IsOptional()
  @IsUUID()
  regimen_id?: string;

  @ApiProperty({ description: 'Razón social' })
  @IsString()
  razon_social: string;

  @ApiProperty({ description: 'RUC', required: false })
  @IsOptional()
  @IsString()
  ruc?: string;

  @ApiProperty({ description: '¿Es la plataforma?', required: false })
  @IsOptional()
  @IsBoolean()
  es_plataforma?: boolean;
}

export class GenerarDeclaracionesRequestDto {
  @ApiProperty({ description: 'Tipo de declaración', required: false, enum: ['IVA', 'IR', 'CUOTA_FIJA'] })
  @IsOptional()
  @IsIn(['IVA', 'IR', 'CUOTA_FIJA'])
  tipo?: string;

  @ApiProperty({ description: 'Inicio del periodo', required: false })
  @IsOptional()
  @IsString()
  inicio?: string;

  @ApiProperty({ description: 'Fin del periodo', required: false })
  @IsOptional()
  @IsString()
  fin?: string;
}

/**
 * Tributacion: GET/POST /api/v1/finanzas/{jurisdicciones,regimenes,sujetos,
 * declaraciones} — regimen fiscal anual y declaraciones mensuales (Ley 822).
 * GET /api/v1/vendedores/me/fiscal — situacion del vendedor (cap. 4.4).
 */
@ApiTags('Finanzas · Tributación')
@Controller('api/v1/finanzas')
@Roles(ROLES.ADMIN)
export class TributacionController {
  constructor(private readonly tributacion: TributacionService) {}

  @Get('jurisdicciones')
  @ApiOperation({ summary: 'Listar jurisdicciones' })
  async jurisdicciones(): Promise<Jurisdiccion[]> {
    return this.tributacion.listarJurisdicciones();
  }

  @Post('jurisdicciones')
  @ApiOperation({ summary: 'Crear una jurisdicción' })
  async crearJurisdiccion(@Body() dto: CrearJurisdiccionRequestDto): Promise<Jurisdiccion> {
    return this.tributacion.crearJurisdiccion({
      codigo_pais: dto.codigo_pais,
      nombre: dto.nombre,
      moneda: dto.moneda,
      simbolo_moneda: dto.simbolo_moneda,
      tasa_iva: dto.tasa_iva_por_mil / 1000,
      tasa_ir: dto.tasa_ir_por_mil / 1000,
      periodicidad_declaracion: dto.periodicidad_declaracion,
      leyes: dto.leyes,
    });
  }

  @Get('regimenes')
  @ApiOperation({ summary: 'Listar regímenes fiscales' })
  @ApiQuery({ name: 'jurisdiccion', required: false })
  async regimenes(@Query('jurisdiccion') jurisdiccion?: string): Promise<RegimenFiscal[]> {
    return this.tributacion.regimenesDe(jurisdiccion);
  }

  @Post('regimenes')
  @ApiOperation({ summary: 'Crear un régimen fiscal' })
  async crearRegimen(@Body() dto: CrearRegimenRequestDto): Promise<RegimenFiscal> {
    const regimen = await this.tributacion.crearRegimen(dto);
    if (!regimen) throw new NotFoundError('Regimen', dto.codigo);
    return regimen;
  }

  @Get('sujetos')
  @ApiOperation({ summary: 'Listar sujetos tributarios' })
  async sujetos(): Promise<SujetoTributario[]> {
    return this.tributacion.sujetos();
  }

  @Post('sujetos')
  @ApiOperation({ summary: 'Registrar un sujeto tributario' })
  async registrarSujeto(@Body() dto: RegistrarSujetoRequestDto): Promise<SujetoTributario> {
    const sujeto = await this.tributacion.registrarSujeto(dto);
    if (!sujeto) throw new NotFoundError('Sujeto', dto.id);
    return sujeto;
  }

  @Post('sujetos/:id/baja')
  @ApiOperation({ summary: 'Dar de baja un sujeto tributario' })
  @ApiParam({ name: 'id', description: 'Id del sujeto' })
  async darDeBaja(@Param('id') id: string): Promise<SujetoTributario> {
    const sujeto = await this.tributacion.darDeBaja(id);
    if (!sujeto) throw new NotFoundError('Sujeto', id);
    return sujeto;
  }

  @Get('declaraciones')
  @ApiOperation({ summary: 'Listar declaraciones' })
  @ApiQuery({ name: 'tipo', required: false })
  @ApiQuery({ name: 'periodo_inicio', required: false })
  @ApiQuery({ name: 'estado', required: false })
  async declaraciones(
    @Query('tipo') tipo?: string,
    @Query('periodo_inicio') periodoInicio?: string,
    @Query('estado') estado?: string,
  ): Promise<Declaracion[]> {
    return this.tributacion.declaraciones({ tipo, periodo_inicio: periodoInicio, estado });
  }

  /** Genera las declaraciones del periodo (por defecto el mes anterior). */
  @Post('declaraciones/generar')
  @ApiOperation({ summary: 'Generar declaraciones del periodo' })
  async generar(@Body() dto: GenerarDeclaracionesRequestDto): Promise<Declaracion[]> {
    const periodo = dto.inicio && dto.fin ? { inicio: dto.inicio, fin: dto.fin } : undefined;
    return this.tributacion.generarDeclaraciones(periodo);
  }

  @Post('declaraciones/:id/presentar')
  @ApiOperation({ summary: 'Presentar una declaración' })
  @ApiParam({ name: 'id', description: 'Id de la declaración' })
  async presentar(@Param('id') id: string): Promise<Declaracion> {
    const declaracion = await this.tributacion.presentar(id);
    if (!declaracion) throw new NotFoundError('Declaracion', id);
    return declaracion;
  }

  @Post('declaraciones/:id/pagar')
  @ApiOperation({ summary: 'Marcar una declaración como pagada' })
  @ApiParam({ name: 'id', description: 'Id de la declaración' })
  async pagar(@Param('id') id: string): Promise<Declaracion> {
    const declaracion = await this.tributacion.marcarPagada(id);
    if (!declaracion) throw new NotFoundError('Declaracion', id);
    return declaracion;
  }
}
