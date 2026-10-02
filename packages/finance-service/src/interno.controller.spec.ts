import 'reflect-metadata';
import { InternoController } from './interno.controller';
import type { TributacionService } from './tributacion/tributacion.service';
import { ClaveInternaGuard } from '@core/shared';

function fabricar(): { tributacion: Record<string, jest.Mock>; controller: InternoController } {
  const tributacion = { sujetoFiscalDe: jest.fn(), declaraciones: jest.fn() };
  const controller = new InternoController(tributacion as unknown as TributacionService);
  return { tributacion, controller };
}

describe('InternoController (internal/finance, R-U-10/11)', () => {
  it('debe devolver la situacion fiscal del usuario', async () => {
    const { tributacion, controller } = fabricar();
    tributacion.sujetoFiscalDe.mockResolvedValue({ id: 's-1' });
    await expect(controller.sujeto('u-1')).resolves.toEqual({ id: 's-1' });
    expect(tributacion.sujetoFiscalDe).toHaveBeenCalledWith('u-1');
  });

  it('debe listar declaraciones con filtros', async () => {
    const { tributacion, controller } = fabricar();
    tributacion.declaraciones.mockResolvedValue([]);
    await controller.declaraciones('IVA', '2026-09-01', 'pendiente');
    expect(tributacion.declaraciones).toHaveBeenCalledWith({
      tipo: 'IVA',
      periodo_inicio: '2026-09-01',
      estado: 'pendiente',
    });
  });

  it('debe proteger las rutas con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternoController) as unknown[];
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});