import type { Mock } from 'vitest';
import 'reflect-metadata';
import { InternoController } from './interno.controller';
import type { TributacionService } from './tributacion/tributacion.service';
import type { ComercioService } from './comercio/comercio.service';
import { ClaveInternaGuard } from '@core/shared';

function fabricar(): {
  tributacion: Record<string, Mock>;
  comercio: Record<string, Mock>;
  controller: InternoController;
} {
  const tributacion = { sujetoFiscalDe: vi.fn(), declaraciones: vi.fn() };
  const comercio = { registrarCreditoOrden: vi.fn(), anularCreditoOrden: vi.fn() };
  const controller = new InternoController(
    tributacion as unknown as TributacionService,
    comercio as unknown as ComercioService,
  );
  return { tributacion, comercio, controller };
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

  it('debe registrar y anular el credito de una orden', async () => {
    const { comercio, controller } = fabricar();
    comercio.registrarCreditoOrden.mockResolvedValue({ credito_id: 'c-1' });
    comercio.anularCreditoOrden.mockResolvedValue(true);

    await expect(
      controller.registrarCredito({
        empresa_id: 'e-1',
        parte_id: 'p-1',
        order_id: 'o-1',
        monto_cents: 100,
        concepto: 'Pedido',
      }),
    ).resolves.toEqual({ credito_id: 'c-1' });
    await expect(controller.anularCredito('o-1')).resolves.toEqual({ ok: true });
  });

  it('debe proteger las rutas con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternoController) as unknown[];
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});