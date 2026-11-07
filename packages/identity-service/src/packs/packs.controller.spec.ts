import { PacksController } from './packs.controller';

describe('PacksController', () => {
  it('debe normalizar la vertical antes de listar packs', async () => {
    const packs = { listar: vi.fn().mockResolvedValue([]) };
    const controller = new PacksController(packs as never);

    await controller.listar({ vertical_id: ' Belleza ' });

    expect(packs.listar).toHaveBeenCalledWith('belleza');
  });

  it('debe normalizar el codigo antes de obtener un pack', async () => {
    const packs = { obtener: vi.fn().mockResolvedValue({ codigo: 'pack_belleza', version: 1 }) };
    const controller = new PacksController(packs as never);

    await controller.obtener(' PACK_BELLEZA ', 1);

    expect(packs.obtener).toHaveBeenCalledWith('pack_belleza', 1);
  });
});
