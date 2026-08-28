import { Controller, Get } from '@nestjs/common';
import { Roles, ROLES } from '@core/shared';
import { ReportesService, ReporteKPI } from './reportes.service';

/** GET /api/v1/admin/reportes — KPIs de ventas e inventario (JWT admin). */
@Controller('api/v1/admin/reportes')
export class ReportesController {
  constructor(private readonly reportes: ReportesService) {}

  @Get()
  @Roles(ROLES.ADMIN)
  async kpis(): Promise<ReporteKPI> {
    return this.reportes.kpis();
  }
}