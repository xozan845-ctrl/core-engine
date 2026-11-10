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

  it('debe listar modelos y verticales', async () => {
    const packs = {
      listarModelos: vi.fn().mockResolvedValue([]),
      listarVerticales: vi.fn().mockResolvedValue([]),
    };
    const controller = new PacksController(packs as never);

    await controller.listarModelos();
    await controller.listarVerticales();

    expect(packs.listarModelos).toHaveBeenCalled();
    expect(packs.listarVerticales).toHaveBeenCalled();
  });

  it('debe completar los defaults al crear un modelo operativo', async () => {
    const packs = { crearModelo: vi.fn().mockResolvedValue({ codigo: 'modelo' }) };
    const controller = new PacksController(packs as never);

    await controller.crearModelo({ codigo: 'modelo', nombre: 'Modelo' });

    expect(packs.crearModelo).toHaveBeenCalledWith({
      codigo: 'modelo',
      nombre: 'Modelo',
      descripcion: '',
      config: {},
    });
  });

  it('debe delegar la creación de verticales y packs', async () => {
    const packs = {
      crearVertical: vi.fn().mockResolvedValue({ codigo: 'vertical' }),
      crear: vi.fn().mockResolvedValue({ codigo: 'pack_vertical', version: 1 }),
    };
    const controller = new PacksController(packs as never);

    await controller.crearVertical({ codigo: 'vertical', nombre: 'Vertical', modelo_operacion: 'm' });
    await controller.crear({ codigo: 'pack_vertical', version: 1, vertical_codigo: 'vertical', nombre: 'Pack' });

    expect(packs.crearVertical).toHaveBeenCalledWith({
      codigo: 'vertical',
      nombre: 'Vertical',
      modelo_operacion: 'm',
    });
    expect(packs.crear).toHaveBeenCalledWith({
      codigo: 'pack_vertical',
      version: 1,
      vertical_codigo: 'vertical',
      nombre: 'Pack',
    });
  });
});
