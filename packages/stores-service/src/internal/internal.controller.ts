import { Controller, Get, Query, UseGuards } from '@nestjs/common';
import { ApiOperation, ApiQuery, ApiTags } from '@nestjs/swagger';
import { ClaveInternaGuard } from '@core/shared';
import { OfertasService } from '../ofertas/ofertas.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos con clave interna.
 */
@ApiTags('Tiendas (interno)')
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(private readonly ofertas: OfertasService) {}

  /** Lote de ofertas por ids (enriquecimiento de precios en el checkout). */
  @Get('ofertas')
  @ApiOperation({ summary: 'Lote de ofertas por ids (servicio→servicio)' })
  @ApiQuery({ name: 'ids', required: false, description: 'Ids de oferta separados por coma' })
  async porIds(@Query('ids') idsRaw?: string) {
    const ids = (idsRaw ?? '').split(',').map((s) => s.trim()).filter(Boolean);
    return this.ofertas.porIds(ids);
  }
}
