import 'reflect-metadata';
import { LiquidacionesController } from './liquidaciones.controller';
import type { LiquidacionesService } from './liquidaciones.service';
import { NotFoundError, ROLES } from '@core/shared';

const liquidacion = { id: 'liq-1', vendedor_id: 'u-v', monto_cents: 100000, estado: 'pendiente' };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };

function fabricar(): { liquidaciones: Record<string, jest.Mock>; controller: LiquidacionesController } {
  const liquidaciones = {
    deVendedor: jest.fn(),
    cerrarPeriodo: jest.fn(),
    cerrarPeriodoAnterior: jest.fn(),
    marcarPagada: jest.fn(),
  };
  const controller = new LiquidacionesController(liquidaciones as unknown as LiquidacionesService);
  return { liquidaciones, controller };
}

describe('LiquidacionesController (api/v1, RN-07, R-U-10/11)', () => {
  it('debe devolver las liquidaciones del vendedor autenticado', async () => {
    const { liquidaciones, controller } = fabricar();
    liquidaciones.deVendedor.mockResolvedValue([liquidacion]);
    await expect(controller.mias(vendedor as never)).resolves.toEqual([liquidacion]);
    expect(liquidaciones.deVendedor).toHaveBeenCalledWith('u-v');
  });

  it('debe cerrar el periodo indicado cuando llegan inicio y fin', async () => {
    const { liquidaciones, controller } = fabricar();
    liquidaciones.cerrarPeriodo.mockResolvedValue([liquidacion]);
    await controller.corteManual('2026-09-01', '2026-09-15');
    expect(liquidaciones.cerrarPeriodo).toHaveBeenCalledWith({ inicio: '2026-09-01', fin: '2026-09-15' });
  });

  it('debe cerrar el periodo anterior cuando no llegan fechas', async () => {
    const { liquidaciones, controller } = fabricar();
    liquidaciones.cerrarPeriodoAnterior.mockResolvedValue([liquidacion]);
    await controller.corteManual(undefined, undefined);
    expect(liquidaciones.cerrarPeriodoAnterior).toHaveBeenCalledWith(expect.any(Date));
  });

  it('debe marcar pagada la liquidacion cuando existe', async () => {
    const { liquidaciones, controller } = fabricar();
    liquidaciones.marcarPagada.mockResolvedValue(liquidacion);
    await expect(controller.pagar('liq-1', 'pagada')).resolves.toEqual(liquidacion);
    expect(liquidaciones.marcarPagada).toHaveBeenCalledWith('liq-1');
  });

  it('debe lanzar NotFoundError cuando la liquidacion a pagar no existe', async () => {
    const { liquidaciones, controller } = fabricar();
    liquidaciones.marcarPagada.mockResolvedValue(null);
    await expect(controller.pagar('liq-x', undefined)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe exigir rol vendedor o admin segun la ruta', () => {
    expect(Reflect.getMetadata('roles_requeridos', LiquidacionesController.prototype.mias)).toEqual([ROLES.VENDEDOR]);
    expect(Reflect.getMetadata('roles_requeridos', LiquidacionesController.prototype.corteManual)).toEqual([ROLES.ADMIN]);
    expect(Reflect.getMetadata('roles_requeridos', LiquidacionesController.prototype.pagar)).toEqual([ROLES.ADMIN]);
  });
});