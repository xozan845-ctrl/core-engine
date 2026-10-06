import { Controller, Get } from '@nestjs/common';
import { ApiOperation, ApiTags } from '@nestjs/swagger';
import { Roles, ROLES } from '@core/shared';
import { ReportesService, ReporteKPI } from './reportes.service';

/** GET /api/v1/admin/reportes — KPIs de ventas e inventario (JWT admin). */
@ApiTags('Comisiones · Reportes')
@Controller('api/v1/admin/reportes')
export class ReportesController {
  constructor(private readonly reportes: ReportesService) {}

  @Get()
  @Roles(ROLES.ADMIN)
  @ApiOperation({ summary: 'KPIs de ventas e inventario (admin)' })
  async kpis(): Promise<ReporteKPI> {
    return this.reportes.kpis();
  }
}
