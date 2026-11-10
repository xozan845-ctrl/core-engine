import { Controller, Get } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiTags } from '@nestjs/swagger';
import { Roles, ROLES, CAPACIDADES, RequiereCapacidades } from '@core/shared';
import { ReportesService, ReporteKPI } from './reportes.service';
import { ReporteKpiResponseDto } from './reportes.response.dto';

/** GET /api/v2/admin/reportes — KPIs de ventas e inventario (JWT admin). */
@ApiTags('Comisiones · Reportes')
@ApiBearerAuth()
@Controller('api/v2/admin/reportes')
@RequiereCapacidades(CAPACIDADES.LIQUIDACIONES)
export class ReportesController {
  constructor(private readonly reportes: ReportesService) {}

  @Get()
  @Roles(ROLES.SUPER_ADMIN, ROLES.ADMIN)
  @ApiOperation({ summary: 'KPIs de ventas e inventario (admin)' })
  @ApiOkResponse({ type: ReporteKpiResponseDto, description: 'KPIs de ventas e inventario' })
  async kpis(): Promise<ReporteKPI> {
    return this.reportes.kpis();
  }
}
