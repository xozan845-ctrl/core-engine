import type { Mock } from 'vitest';
import 'reflect-metadata';
import { RegistrosController } from './registros.controller';
import type { RegistrosService } from './registros.service';
import { ROLES } from '@core/shared';

const actor = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 'e-1' };
const registro = { id: 'r-1', tipo: 'receta' };

function fabricar(): { servicio: Record<string, Mock>; controller: RegistrosController } {
  const servicio = {
    listar: vi.fn(),
    crear: vi.fn(),
    obtener: vi.fn(),
    actualizar: vi.fn(),
    eliminar: vi.fn(),
  };
  return {
    servicio,
    controller: new RegistrosController(servicio as unknown as RegistrosService),
  };
}

describe('RegistrosController (partes)', () => {
  it('lista con tipo y limite convertido a numero', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([registro]);
    await expect(
      controller.listar('e-1', 'p-1', 'receta', '20', actor as never),
    ).resolves.toEqual([registro]);
    expect(servicio.listar).toHaveBeenCalledWith('e-1', 'p-1', actor, 'receta', 20);
  });

  it('delega el alta y la consulta', async () => {
    const { servicio, controller } = fabricar();
    servicio.crear.mockResolvedValue(registro);
    servicio.obtener.mockResolvedValue(registro);

    await controller.crear('e-1', 'p-1', { tipo: 'receta' } as never, actor as never);
    await controller.obtener('e-1', 'p-1', 'r-1', actor as never);

    expect(servicio.crear).toHaveBeenCalledWith('e-1', 'p-1', { tipo: 'receta' }, actor);
    expect(servicio.obtener).toHaveBeenCalledWith('e-1', 'p-1', 'r-1', actor);
  });

  it('delega la actualizacion y el archivado', async () => {
    const { servicio, controller } = fabricar();
    servicio.actualizar.mockResolvedValue(registro);
    servicio.eliminar.mockResolvedValue({ ok: true });

    await controller.actualizar('e-1', 'p-1', 'r-1', { titulo: 'x' } as never, actor as never);
    await expect(controller.eliminar('e-1', 'p-1', 'r-1', actor as never)).resolves.toEqual({
      ok: true,
    });

    expect(servicio.actualizar).toHaveBeenCalledWith('e-1', 'p-1', 'r-1', { titulo: 'x' }, actor);
    expect(servicio.eliminar).toHaveBeenCalledWith('e-1', 'p-1', 'r-1', actor);
  });
});
