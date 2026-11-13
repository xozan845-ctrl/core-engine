import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CAPABILITIES_KEY, CAPACIDADES, ROLES } from '@core/shared';
import { EntregasDigitalesController } from './entregas-digitales.controller';
import type { EntregasDigitalesService } from './entregas-digitales.service';

const ORDER = '11111111-1111-4111-8111-111111111111';
const TOKEN = '22222222-2222-4222-8222-222222222222';
const comprador = { user_id: 'u-1', email: 'c@core.local', rol: ROLES.COMPRADOR };

function fabricar(): { servicio: Record<string, Mock>; controller: EntregasDigitalesController } {
  const servicio = { listarPorOrden: vi.fn(), descargar: vi.fn() };
  return {
    servicio,
    controller: new EntregasDigitalesController(servicio as unknown as EntregasDigitalesService),
  };
}

describe('EntregasDigitalesController', () => {
  it('exige la capacidad de entrega digital y los roles de lectura', () => {
    expect(Reflect.getMetadata(CAPABILITIES_KEY, EntregasDigitalesController)).toEqual([
      CAPACIDADES.FULFILLMENT_DIGITAL,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', EntregasDigitalesController)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
      ROLES.COMPRADOR,
    ]);
  });

  it('delega listado y descarga con el usuario autenticado', async () => {
    const { servicio, controller } = fabricar();
    servicio.listarPorOrden.mockResolvedValue([]);
    servicio.descargar.mockResolvedValue({ token: TOKEN, descargas: 1 });

    await expect(controller.listar(ORDER, comprador as never)).resolves.toEqual([]);
    expect(servicio.listarPorOrden).toHaveBeenCalledWith(ORDER, comprador);

    await expect(controller.descargar(TOKEN, comprador as never)).resolves.toMatchObject({
      descargas: 1,
    });
    expect(servicio.descargar).toHaveBeenCalledWith(TOKEN, comprador);
  });
});
