import { Body, Controller, Get, Param, Post, Query } from '@nestjs/common';
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
  @IsString()
  @MinLength(2)
  codigo_pais: string;

  @IsString()
  nombre: string;

  @IsString()
  moneda: string;

  @IsString()
  simbolo_moneda: string;

  @IsInt()
  @Min(0)
  tasa_iva_por_mil: number;

  @IsInt()
  @Min(0)
  tasa_ir_por_mil: number;

  @IsOptional()
  @IsString()
  periodicidad_declaracion?: string;

  @IsOptional()
  leyes?: Record<string, string>;
}

export class CrearRegimenRequestDto {
  @IsString()
  @MinLength(2)
  jurisdiccion: string;

  @IsString()
  codigo: string;

  @IsString()
  nombre: string;

  @IsOptional()
  @IsString()
  descripcion?: string;

  @IsOptional()
  @IsString()
  periodicidad?: string;

  @IsOptional()
  @IsInt()
  condicion_ingresos_anuales_cents?: number;
}

export class RegistrarSujetoRequestDto {
  @IsUUID()
  id: string;

  @IsOptional()
  @IsString()
  jurisdiccion?: string;

  @IsOptional()
  @IsUUID()
  regimen_id?: string;

  @IsString()
  razon_social: string;

  @IsOptional()
  @IsString()
  ruc?: string;

  @IsOptional()
  @IsBoolean()
  es_plataforma?: boolean;
}

export class GenerarDeclaracionesRequestDto {
  @IsOptional()
  @IsIn(['IVA', 'IR', 'CUOTA_FIJA'])
  tipo?: string;

  @IsOptional()
  @IsString()
  inicio?: string;

  @IsOptional()
  @IsString()
  fin?: string;
}

/**
 * Tributacion: GET/POST /api/v1/finanzas/{jurisdicciones,regimenes,sujetos,
 * declaraciones} — regimen fiscal anual y declaraciones mensuales (Ley 822).
 * GET /api/v1/vendedores/me/fiscal — situacion del vendedor (cap. 4.4).
 */
@Controller('api/v1/finanzas')
@Roles(ROLES.ADMIN)
export class TributacionController {
  constructor(private readonly tributacion: TributacionService) {}

  @Get('jurisdicciones')
  async jurisdicciones(): Promise<Jurisdiccion[]> {
    return this.tributacion.listarJurisdicciones();
  }

  @Post('jurisdicciones')
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
  async regimenes(@Query('jurisdiccion') jurisdiccion?: string): Promise<RegimenFiscal[]> {
    return this.tributacion.regimenesDe(jurisdiccion);
  }

  @Post('regimenes')
  async crearRegimen(@Body() dto: CrearRegimenRequestDto): Promise<RegimenFiscal> {
    const regimen = await this.tributacion.crearRegimen(dto);
    if (!regimen) throw new NotFoundError('Regimen', dto.codigo);
    return regimen;
  }

  @Get('sujetos')
  async sujetos(): Promise<SujetoTributario[]> {
    return this.tributacion.sujetos();
  }

  @Post('sujetos')
  async registrarSujeto(@Body() dto: RegistrarSujetoRequestDto): Promise<SujetoTributario> {
    const sujeto = await this.tributacion.registrarSujeto(dto);
    if (!sujeto) throw new NotFoundError('Sujeto', dto.id);
    return sujeto;
  }

  @Post('sujetos/:id/baja')
  async darDeBaja(@Param('id') id: string): Promise<SujetoTributario> {
    const sujeto = await this.tributacion.darDeBaja(id);
    if (!sujeto) throw new NotFoundError('Sujeto', id);
    return sujeto;
  }

  @Get('declaraciones')
  async declaraciones(
    @Query('tipo') tipo?: string,
    @Query('periodo_inicio') periodoInicio?: string,
    @Query('estado') estado?: string,
  ): Promise<Declaracion[]> {
    return this.tributacion.declaraciones({ tipo, periodo_inicio: periodoInicio, estado });
  }

  /** Genera las declaraciones del periodo (por defecto el mes anterior). */
  @Post('declaraciones/generar')
  async generar(@Body() dto: GenerarDeclaracionesRequestDto): Promise<Declaracion[]> {
    const periodo = dto.inicio && dto.fin ? { inicio: dto.inicio, fin: dto.fin } : undefined;
    return this.tributacion.generarDeclaraciones(periodo);
  }

  @Post('declaraciones/:id/presentar')
  async presentar(@Param('id') id: string): Promise<Declaracion> {
    const declaracion = await this.tributacion.presentar(id);
    if (!declaracion) throw new NotFoundError('Declaracion', id);
    return declaracion;
  }

  @Post('declaraciones/:id/pagar')
  async pagar(@Param('id') id: string): Promise<Declaracion> {
    const declaracion = await this.tributacion.marcarPagada(id);
    if (!declaracion) throw new NotFoundError('Declaracion', id);
    return declaracion;
  }
}
