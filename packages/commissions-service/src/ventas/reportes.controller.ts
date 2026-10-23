import { Controller, Get } from '@nestjs/common';
import { ApiBearerAuth, ApiOkResponse, ApiOperation, ApiTags } from '@nestjs/swagger';
import { Roles, ROLES } from '@core/shared';
import { ReportesService, ReporteKPI } from './reportes.service';
import { ReporteKpiResponseDto } from './reportes.response.dto';

/** GET /api/v1/admin/reportes — KPIs de ventas e inventario (JWT admin). */
@ApiTags('Comisiones · Reportes')
@ApiBearerAuth()
@Controller('api/v1/admin/reportes')
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
