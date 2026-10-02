import 'reflect-metadata';
import { FacturacionController } from './facturacion.controller';
import type { FacturacionService } from './facturacion.service';
import { NotFoundError, ROLES } from '@core/shared';

const comprobante = { id: 'c-1', serie: 'A', numero: '1', tipo: 'FACTURA', total_cents: 1000 };

function fabricar(): { facturacion: Record<string, jest.Mock>; controller: FacturacionController } {
  const facturacion = { listar: jest.fn(), series: jest.fn(), emitir: jest.fn(), anular: jest.fn() };
  const controller = new FacturacionController(facturacion as unknown as FacturacionService);
  return { facturacion, controller };
}

describe('FacturacionController (api/v1/finanzas/comprobantes, Ley 822, R-U-10/11)', () => {
  it('debe listar comprobantes con rango y estado', async () => {
    const { facturacion, controller } = fabricar();
    facturacion.listar.mockResolvedValue([comprobante]);
    await expect(controller.listar('2026-09-01', '2026-09-30', 'emitida')).resolves.toEqual([comprobante]);
    expect(facturacion.listar).toHaveBeenCalledWith('2026-09-01', '2026-09-30', 'emitida');
  });

  it('debe devolver las series', async () => {
    const { facturacion, controller } = fabricar();
    facturacion.series.mockResolvedValue([{ serie: 'A', siguiente: 2 }]);
    await expect(controller.series()).resolves.toEqual([{ serie: 'A', siguiente: 2 }]);
  });

  it('debe emitir el comprobante separando tipo/orden y datos fiscales', async () => {
    const { facturacion, controller } = fabricar();
    facturacion.emitir.mockResolvedValue(comprobante);
    const dto = { tipo: 'FACTURA', orden_id: 'o-1', razon_social: 'ACME', ruc: 'J0310000' };
    await expect(controller.emitir(dto as never)).resolves.toEqual(comprobante);
    expect(facturacion.emitir).toHaveBeenCalledWith('FACTURA', 'o-1', {
      cliente_id: undefined,
      razon_social: 'ACME',
      ruc: 'J0310000',
    });
  });

  it('debe anular el comprobante cuando existe y lanzar NotFoundError cuando no', async () => {
    const { facturacion, controller } = fabricar();
    facturacion.anular.mockResolvedValue(comprobante);
    await expect(controller.anular('c-1')).resolves.toEqual(comprobante);
    facturacion.anular.mockResolvedValue(null);
    await expect(controller.anular('c-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe exigir rol admin a nivel de controlador', () => {
    expect(Reflect.getMetadata('roles_requeridos', FacturacionController)).toEqual([ROLES.ADMIN]);
  });
});