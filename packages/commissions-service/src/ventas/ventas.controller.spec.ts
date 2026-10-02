import 'reflect-metadata';
import { VentasController } from './ventas.controller';
import type { VentasService } from './ventas.service';
import { ROLES } from '@core/shared';

const estado = { total_ventas_cents: 115000, comisiones_cents: 13800, liquidado_cents: 0 };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };

function fabricar(): { ventas: Record<string, jest.Mock>; controller: VentasController } {
  const ventas = { estadoCuenta: jest.fn() };
  const controller = new VentasController(ventas as unknown as VentasService);
  return { ventas, controller };
}

describe('VentasController (api/v1/vendedores/me/ventas, R-U-10/11)', () => {
  it('debe devolver el estado de cuenta del vendedor autenticado', async () => {
    const { ventas, controller } = fabricar();
    ventas.estadoCuenta.mockResolvedValue(estado);
    await expect(controller.estado(vendedor as never)).resolves.toEqual(estado);
    expect(ventas.estadoCuenta).toHaveBeenCalledWith('u-v');
  });

  it('debe exigir rol vendedor', () => {
    expect(Reflect.getMetadata('roles_requeridos', VentasController.prototype.estado)).toEqual([ROLES.VENDEDOR]);
  });
});