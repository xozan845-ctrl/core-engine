import { ROLES } from '@core/shared';
import { ExtensionesController } from './extensiones.controller';

const actor = { user_id: 'u-1', email: 'e@t', rol: ROLES.EMPRESARIO, tenant_id: 'e-1' };

function fabricar() {
  const extensiones = {
    listar: vi.fn().mockResolvedValue([]),
    crear: vi.fn().mockResolvedValue({}),
    cambiarEstado: vi.fn().mockResolvedValue({}),
    validar: vi.fn().mockResolvedValue({ valido: true, errores: [] }),
    obtenerValores: vi.fn().mockResolvedValue(null),
    guardarValores: vi.fn().mockResolvedValue({}),
    eliminarValores: vi.fn().mockResolvedValue(undefined),
  };
  const controller = new ExtensionesController(extensiones as never);
  return { extensiones, controller };
}

describe('ExtensionesController (R-U-10/11)', () => {
  it('debe delegar el listado de extensiones', async () => {
    const { extensiones, controller } = fabricar();
    await controller.listar('e-1', actor);
    expect(extensiones.listar).toHaveBeenCalledWith('e-1', actor);
  });

  it('debe delegar la creación de una extensión', async () => {
    const { extensiones, controller } = fabricar();
    const dto = { codigo: 'ficha', nombre: 'Ficha', recurso: 'cliente', esquema: {} };
    await controller.crear('e-1', dto, actor);
    expect(extensiones.crear).toHaveBeenCalledWith('e-1', dto, actor);
  });

  it('debe delegar el cambio de estado', async () => {
    const { extensiones, controller } = fabricar();
    await controller.cambiarEstado('e-1', 'ficha', { activo: false }, actor);
    expect(extensiones.cambiarEstado).toHaveBeenCalledWith('e-1', 'ficha', false, actor);
  });

  it('debe delegar la validación de datos', async () => {
    const { extensiones, controller } = fabricar();
    await controller.validar('e-1', 'ficha', { datos: { a: 1 } }, actor);
    expect(extensiones.validar).toHaveBeenCalledWith('e-1', 'ficha', { a: 1 }, actor);
  });

  it('debe consultar, guardar y eliminar valores', async () => {
    const { extensiones, controller } = fabricar();
    await controller.obtenerValores('e-1', 'ficha', 'r-1', actor);
    await controller.guardarValores('e-1', 'ficha', 'r-1', { valores: { a: 1 } }, actor);
    await expect(controller.eliminarValores('e-1', 'ficha', 'r-1', actor)).resolves.toEqual({
      ok: true,
    });
    expect(extensiones.obtenerValores).toHaveBeenCalledWith('e-1', 'ficha', 'r-1', actor);
    expect(extensiones.guardarValores).toHaveBeenCalledWith('e-1', 'ficha', 'r-1', { a: 1 }, actor);
    expect(extensiones.eliminarValores).toHaveBeenCalledWith('e-1', 'ficha', 'r-1', actor);
  });
});
