import type { Mock } from 'vitest';
import 'reflect-metadata';
import { TarifasController } from './tarifas.controller';
import type { TarifasService } from './tarifas.service';
import { ROLES } from '@core/shared';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };

function fabricar(): { servicio: Record<string, Mock>; controller: TarifasController } {
  const servicio = {
    listar: vi.fn(),
    crear: vi.fn(),
    actualizar: vi.fn(),
    eliminar: vi.fn(),
    cotizar: vi.fn(),
  };
  return { servicio, controller: new TarifasController(servicio as unknown as TarifasService) };
}

describe('TarifasController (catálogo)', () => {
  it('usa el tenant del empresario y delega el alta', async () => {
    const { servicio, controller } = fabricar();
    servicio.crear.mockResolvedValue({ id: 't-1' });
    await controller.crear(
      'i-1',
      { desde: '2026-12-20', hasta: '2026-12-31', precio_cents: 100 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crear).toHaveBeenCalledWith('t-1', 'i-1', {
      desde: '2026-12-20',
      hasta: '2026-12-31',
      precio_cents: 100,
    });
  });

  it('exige empresa_id al admin global', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([]);
    await expect(controller.listar('i-1', 'e-9', admin as never)).resolves.toEqual([]);
    expect(servicio.listar).toHaveBeenCalledWith('e-9', 'i-1');
    expect(() => controller.listar('i-1', undefined, admin as never)).toThrow(/empresa_id/);
  });

  it('delega cotización, actualización y borrado', async () => {
    const { servicio, controller } = fabricar();
    servicio.cotizar.mockResolvedValue({ total_cents: 100 });
    servicio.actualizar.mockResolvedValue({ id: 't-1' });
    servicio.eliminar.mockResolvedValue({ ok: true });

    await controller.cotizar('i-1', '2026-12-18', '2026-12-22', undefined, empresario as never);
    await controller.actualizar('i-1', 't-1', { precio_cents: 200 } as never, undefined, empresario as never);
    await expect(controller.eliminar('i-1', 't-1', undefined, empresario as never)).resolves.toEqual({
      ok: true,
    });

    expect(servicio.cotizar).toHaveBeenCalledWith('t-1', 'i-1', '2026-12-18', '2026-12-22');
    expect(servicio.actualizar).toHaveBeenCalledWith('t-1', 'i-1', 't-1', { precio_cents: 200 });
    expect(servicio.eliminar).toHaveBeenCalledWith('t-1', 'i-1', 't-1');
  });
});
