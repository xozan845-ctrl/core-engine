import 'reflect-metadata';
import { InternalController } from './internal.controller';
import type { OfertasService } from '../ofertas/ofertas.service';
import { ClaveInternaGuard } from '@core/shared';

function fabricar(): { ofertas: Record<string, jest.Mock>; controller: InternalController } {
  const ofertas = { porIds: jest.fn() };
  const controller = new InternalController(ofertas as unknown as OfertasService);
  return { ofertas, controller };
}

describe('InternalController (internal, R-U-10/11)', () => {
  it('debe dividir los ids de ofertas en lista limpia y consultarlas por lote', async () => {
    const { ofertas, controller } = fabricar();
    ofertas.porIds.mockResolvedValue([{ id: 'of-1' }]);
    await controller.porIds('of-1, , of-2');
    expect(ofertas.porIds).toHaveBeenCalledWith(['of-1', 'of-2']);
  });

  it('debe consultar la lista vacia cuando no llegan ids', async () => {
    const { ofertas, controller } = fabricar();
    ofertas.porIds.mockResolvedValue([]);
    await controller.porIds(undefined);
    expect(ofertas.porIds).toHaveBeenCalledWith([]);
  });

  it('debe proteger las rutas con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternalController) as unknown[];
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});