import { Controller, Get, Query, UseGuards } from '@nestjs/common';
import { ApiOkResponse, ApiOperation, ApiQuery, ApiSecurity, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard } from '@core/shared';
import { OfertasService } from '../ofertas/ofertas.service';
import { OfertaResponseDto } from '../ofertas/ofertas.response.dto';

/**
 * Endpoints internos (servicio -> servicio) protegidos con clave interna.
 */
@ApiTags('Tiendas (interno)')
@ApiSecurity('internalKey')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(private readonly ofertas: OfertasService) {}

  /** Lote de ofertas por ids (enriquecimiento de precios en el checkout). */
  @Get('ofertas')
  @ApiOperation({ summary: 'Lote de ofertas por ids (servicio→servicio)' })
  @ApiQuery({ name: 'ids', required: false, description: 'Ids de oferta separados por coma' })
  @ApiOkResponse({ type: [OfertaResponseDto], description: 'Ofertas encontradas' })
  async porIds(@Query('ids') idsRaw?: string) {
    const ids = (idsRaw ?? '').split(',').map((s) => s.trim()).filter(Boolean);
    return this.ofertas.porIds(ids);
  }
}
