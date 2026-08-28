import { Controller, Get, Query, UseGuards } from '@nestjs/common';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';
import { OfertasService } from '../ofertas/ofertas.service';

/**
 * Endpoints internos (servicio -> servicio) protegidos por clave interna.
 */
@Controller('internal')
@UseGuards(ClaveInternaGuard)
export class InternalController {
  constructor(private readonly ofertas: OfertasService) {}

  /** Lote de ofertas por ids (enriquecimiento de precios en el checkout). */
  @Get('ofertas')
  async porIds(@Query('ids') idsRaw?: string) {
    const ids = (idsRaw ?? '').split(',').map((s) => s.trim()).filter(Boolean);
    return this.ofertas.porIds(ids);
  }
}