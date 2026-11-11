import type { Mock } from 'vitest';
import 'reflect-metadata';
import { ComponentesController } from './componentes.controller';
import type { ComponentesService } from './componentes.service';
import { ROLES } from '@core/shared';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };

function fabricar(): { servicio: Record<string, Mock>; controller: ComponentesController } {
  const servicio = {
    listar: vi.fn(),
    crear: vi.fn(),
    actualizar: vi.fn(),
    eliminar: vi.fn(),
  };
  return { servicio, controller: new ComponentesController(servicio as unknown as ComponentesService) };
}

describe('ComponentesController (catálogo)', () => {
  it('usa el tenant del empresario y delega el alta', async () => {
    const { servicio, controller } = fabricar();
    servicio.crear.mockResolvedValue({ id: 'c-1' });
    await controller.crear(
      'i-1',
      { componente_item_id: 'c-2', cantidad: 2 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crear).toHaveBeenCalledWith('t-1', 'i-1', {
      componente_item_id: 'c-2',
      cantidad: 2,
    });
  });

  it('exige empresa_id al admin global y rechaza otra empresa', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([]);
    await expect(controller.listar('i-1', 'e-9', admin as never)).resolves.toEqual([]);
    expect(servicio.listar).toHaveBeenCalledWith('e-9', 'i-1');
    expect(() => controller.listar('i-1', undefined, admin as never)).toThrow(/empresa_id/);
    expect(() => controller.listar('i-1', 'otra', empresario as never)).toThrow(
      /No puedes operar otra empresa/,
    );
  });

  it('delega actualización y borrado', async () => {
    const { servicio, controller } = fabricar();
    servicio.actualizar.mockResolvedValue({ id: 'c-1' });
    servicio.eliminar.mockResolvedValue({ ok: true });

    await controller.actualizar('i-1', 'c-1', { cantidad: 3 } as never, undefined, empresario as never);
    await expect(controller.eliminar('i-1', 'c-1', undefined, empresario as never)).resolves.toEqual({
      ok: true,
    });
    expect(servicio.actualizar).toHaveBeenCalledWith('t-1', 'i-1', 'c-1', { cantidad: 3 });
    expect(servicio.eliminar).toHaveBeenCalledWith('t-1', 'i-1', 'c-1');
  });
});
