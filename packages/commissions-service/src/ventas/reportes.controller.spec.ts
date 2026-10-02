import 'reflect-metadata';
import { ReportesController } from './reportes.controller';
import type { ReportesService } from './reportes.service';
import { ROLES } from '@core/shared';

const kpis = { ventas_totales: 10, monto_total_cents: 1150000, ticket_promedio_cents: 115000 };

function fabricar(): { reportes: Record<string, jest.Mock>; controller: ReportesController } {
  const reportes = { kpis: jest.fn() };
  const controller = new ReportesController(reportes as unknown as ReportesService);
  return { reportes, controller };
}

describe('ReportesController (api/v1/admin/reportes, R-U-10/11)', () => {
  it('debe devolver los KPIs que entrega el servicio', async () => {
    const { reportes, controller } = fabricar();
    reportes.kpis.mockResolvedValue(kpis);
    await expect(controller.kpis()).resolves.toEqual(kpis);
    expect(reportes.kpis).toHaveBeenCalledTimes(1);
  });

  it('debe exigir rol admin', () => {
    expect(Reflect.getMetadata('roles_requeridos', ReportesController.prototype.kpis)).toEqual([ROLES.ADMIN]);
  });
});