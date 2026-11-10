import { ROLES } from '@core/shared';
import { CapacidadesController } from './capacidades.controller';

const actor = { user_id: 'u-1', email: 'e@t', rol: ROLES.EMPRESARIO, tenant_id: 'e-1' };

describe('CapacidadesController (R-U-10/11)', () => {
  it('debe listar las capacidades de la empresa del actor', async () => {
    const capacidades = { listar: vi.fn().mockResolvedValue([]), actualizar: vi.fn() };
    const controller = new CapacidadesController(capacidades as never);

    await controller.listar('e-1', actor);

    expect(capacidades.listar).toHaveBeenCalledWith('e-1', actor);
  });

  it('debe normalizar el código de capacidad antes de actualizar', async () => {
    const capacidades = {
      listar: vi.fn(),
      actualizar: vi.fn().mockResolvedValue({ capacidad_codigo: 'agenda_recursos', activo: true }),
    };
    const controller = new CapacidadesController(capacidades as never);

    await controller.actualizar('e-1', ' Agenda_Recursos ', { activo: true }, actor);

    expect(capacidades.actualizar).toHaveBeenCalledWith(
      'e-1',
      'agenda_recursos',
      true,
      undefined,
      actor,
    );
  });

  it('debe propagar la configuración de la capacidad', async () => {
    const capacidades = {
      listar: vi.fn(),
      actualizar: vi.fn().mockResolvedValue({}),
    };
    const controller = new CapacidadesController(capacidades as never);

    await controller.actualizar('e-1', 'agenda', { activo: false, config: { max: 5 } }, actor);

    expect(capacidades.actualizar).toHaveBeenCalledWith(
      'e-1',
      'agenda',
      false,
      { max: 5 },
      actor,
    );
  });
});
