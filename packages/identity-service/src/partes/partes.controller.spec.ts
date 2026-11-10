import { ROLES } from '@core/shared';
import { PartesController } from './partes.controller';

const actor = { user_id: 'u-1', email: 'e@t', rol: ROLES.EMPRESARIO, tenant_id: 'e-1' };

function fabricar() {
  const partes = {
    listar: vi.fn().mockResolvedValue([]),
    crear: vi.fn().mockResolvedValue({ id: 'p-1' }),
    obtener: vi.fn().mockResolvedValue({ id: 'p-1' }),
    actualizar: vi.fn().mockResolvedValue({ id: 'p-1' }),
    eliminar: vi.fn().mockResolvedValue({ ok: true }),
    listarContactos: vi.fn().mockResolvedValue([]),
    crearContacto: vi.fn().mockResolvedValue({ id: 'c-1' }),
    actualizarContacto: vi.fn().mockResolvedValue({ id: 'c-1' }),
    eliminarContacto: vi.fn().mockResolvedValue({ ok: true }),
    listarDirecciones: vi.fn().mockResolvedValue([]),
    crearDireccion: vi.fn().mockResolvedValue({ id: 'd-1' }),
    actualizarDireccion: vi.fn().mockResolvedValue({ id: 'd-1' }),
    eliminarDireccion: vi.fn().mockResolvedValue({ ok: true }),
    listarRelaciones: vi.fn().mockResolvedValue([]),
    crearRelacion: vi.fn().mockResolvedValue({ id: 'r-1' }),
    eliminarRelacion: vi.fn().mockResolvedValue({ ok: true }),
  };
  const controller = new PartesController(partes as never);
  return { partes, controller };
}

describe('PartesController (R-U-10/11)', () => {
  it('debe convertir el límite de texto a número al listar', async () => {
    const { partes, controller } = fabricar();
    await controller.listar('e-1', 'ana', 'persona', '25', actor);
    expect(partes.listar).toHaveBeenCalledWith(
      'e-1',
      { q: 'ana', tipo: 'persona', limite: 25 },
      actor,
    );
  });

  it('debe dejar el límite indefinido cuando no llega', async () => {
    const { partes, controller } = fabricar();
    await controller.listar('e-1', undefined, undefined, undefined, actor);
    expect(partes.listar).toHaveBeenCalledWith(
      'e-1',
      { q: undefined, tipo: undefined, limite: undefined },
      actor,
    );
  });

  it('debe delegar el alta, consulta, actualización y borrado de partes', async () => {
    const { partes, controller } = fabricar();
    const dto = { tipo: 'persona' as const, nombre: 'Ana' };
    await controller.crear('e-1', dto, actor);
    await controller.obtener('e-1', 'p-1', actor);
    await controller.actualizar('e-1', 'p-1', { nombre: 'Ana María' }, actor);
    await controller.eliminar('e-1', 'p-1', actor);
    expect(partes.crear).toHaveBeenCalledWith('e-1', dto, actor);
    expect(partes.obtener).toHaveBeenCalledWith('e-1', 'p-1', actor);
    expect(partes.actualizar).toHaveBeenCalledWith('e-1', 'p-1', { nombre: 'Ana María' }, actor);
    expect(partes.eliminar).toHaveBeenCalledWith('e-1', 'p-1', actor);
  });

  it('debe delegar los contactos de la parte', async () => {
    const { partes, controller } = fabricar();
    await controller.listarContactos('e-1', 'p-1', actor);
    await controller.crearContacto(
      'e-1',
      'p-1',
      { tipo: 'email' as const, valor: 'a@b.c' },
      actor,
    );
    await controller.actualizarContacto('e-1', 'p-1', 'c-1', { valor: 'x@y.z' }, actor);
    await controller.eliminarContacto('e-1', 'p-1', 'c-1', actor);
    expect(partes.listarContactos).toHaveBeenCalledWith('e-1', 'p-1', actor);
    expect(partes.crearContacto).toHaveBeenCalledWith(
      'e-1',
      'p-1',
      { tipo: 'email', valor: 'a@b.c' },
      actor,
    );
    expect(partes.actualizarContacto).toHaveBeenCalledWith(
      'e-1',
      'p-1',
      'c-1',
      { valor: 'x@y.z' },
      actor,
    );
    expect(partes.eliminarContacto).toHaveBeenCalledWith('e-1', 'p-1', 'c-1', actor);
  });

  it('debe delegar las direcciones de la parte', async () => {
    const { partes, controller } = fabricar();
    await controller.listarDirecciones('e-1', 'p-1', actor);
    await controller.crearDireccion(
      'e-1',
      'p-1',
      { tipo: 'fiscal' as const, linea_1: 'Calle 1' },
      actor,
    );
    await controller.actualizarDireccion('e-1', 'p-1', 'd-1', { ciudad: 'Managua' }, actor);
    await controller.eliminarDireccion('e-1', 'p-1', 'd-1', actor);
    expect(partes.listarDirecciones).toHaveBeenCalledWith('e-1', 'p-1', actor);
    expect(partes.crearDireccion).toHaveBeenCalledWith(
      'e-1',
      'p-1',
      { tipo: 'fiscal', linea_1: 'Calle 1' },
      actor,
    );
    expect(partes.actualizarDireccion).toHaveBeenCalledWith(
      'e-1',
      'p-1',
      'd-1',
      { ciudad: 'Managua' },
      actor,
    );
    expect(partes.eliminarDireccion).toHaveBeenCalledWith('e-1', 'p-1', 'd-1', actor);
  });

  it('debe delegar las relaciones y extraer los campos del DTO', async () => {
    const { partes, controller } = fabricar();
    await controller.listarRelaciones('e-1', 'p-1', actor);
    await controller.crearRelacion(
      'e-1',
      'p-1',
      { parte_destino_id: 'p-2', tipo: 'cliente', metadata: { origen: 'manual' } },
      actor,
    );
    await controller.eliminarRelacion('e-1', 'p-1', 'r-1', actor);
    expect(partes.listarRelaciones).toHaveBeenCalledWith('e-1', 'p-1', actor);
    expect(partes.crearRelacion).toHaveBeenCalledWith(
      'e-1',
      'p-1',
      'p-2',
      'cliente',
      { origen: 'manual' },
      actor,
    );
    expect(partes.eliminarRelacion).toHaveBeenCalledWith('e-1', 'p-1', 'r-1', actor);
  });
});
