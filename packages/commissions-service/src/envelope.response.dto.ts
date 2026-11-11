import { ApiProperty } from '@nestjs/swagger';
import {
  LiquidacionEmpresaResponseDto,
  LiquidacionResponseDto,
} from './liquidaciones/liquidaciones.response.dto';
import { ReporteKpiResponseDto } from './ventas/reportes.response.dto';
import {
  EstadoCuentaEmpresaResponseDto,
  EstadoCuentaResponseDto,
} from './ventas/ventas.response.dto';

export class MetaRespuestaResponseDto {
  @ApiProperty({ description: 'Timestamp ISO 8601 UTC', example: '2026-11-11T00:00:00.000Z' })
  timestamp: string;

  @ApiProperty({ description: 'Ruta solicitada', example: '/api/v2/admin/reportes' })
  path: string;

  @ApiProperty({ description: 'Método HTTP', example: 'GET' })
  method: string;

  @ApiProperty({ description: 'Código HTTP de la respuesta', example: 200 })
  statusCode: number;
}

export class EnvelopeEstadoCuentaResponseDto {
  @ApiProperty({ type: EstadoCuentaResponseDto }) data: EstadoCuentaResponseDto;
  @ApiProperty({ type: MetaRespuestaResponseDto }) meta: MetaRespuestaResponseDto;
}

export class EnvelopeEstadoCuentaEmpresaResponseDto {
  @ApiProperty({ type: EstadoCuentaEmpresaResponseDto }) data: EstadoCuentaEmpresaResponseDto;
  @ApiProperty({ type: MetaRespuestaResponseDto }) meta: MetaRespuestaResponseDto;
}

export class EnvelopeReporteKpiResponseDto {
  @ApiProperty({ type: ReporteKpiResponseDto }) data: ReporteKpiResponseDto;
  @ApiProperty({ type: MetaRespuestaResponseDto }) meta: MetaRespuestaResponseDto;
}

export class EnvelopeLiquidacionesResponseDto {
  @ApiProperty({ type: () => [LiquidacionResponseDto] }) data: LiquidacionResponseDto[];
  @ApiProperty({ type: MetaRespuestaResponseDto }) meta: MetaRespuestaResponseDto;
}

export class EnvelopeLiquidacionResponseDto {
  @ApiProperty({ type: LiquidacionResponseDto }) data: LiquidacionResponseDto;
  @ApiProperty({ type: MetaRespuestaResponseDto }) meta: MetaRespuestaResponseDto;
}

export class EnvelopeLiquidacionesEmpresaResponseDto {
  @ApiProperty({ type: () => [LiquidacionEmpresaResponseDto] })
  data: LiquidacionEmpresaResponseDto[];
  @ApiProperty({ type: MetaRespuestaResponseDto }) meta: MetaRespuestaResponseDto;
}
