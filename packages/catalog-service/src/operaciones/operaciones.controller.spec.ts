import { DomainError, NotFoundError, ROLES } from '@core/shared';
import { OperacionesController } from './operaciones.controller';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';

function crear() {
  const servicio = {
    obtenerConfiguracion: vi.fn().mockResolvedValue({}),
    guardarConfiguracion: vi.fn().mockResolvedValue({}),
    importarProductos: vi.fn().mockResolvedValue({ importados: 1, omitidos: 0, errores: [] }),
    escanear: vi.fn().mockResolvedValue({ tipo: 'producto' }),
    listarCombos: vi.fn().mockResolvedValue({ items: [] }),
    crearCombo: vi.fn().mockResolvedValue({ id: 'c1' }),
    obtenerCombo: vi.fn().mockResolvedValue({ id: 'c1' }),
    actualizarCombo: vi.fn().mockResolvedValue({ id: 'c1' }),
    eliminarCombo: vi.fn().mockResolvedValue(undefined),
    listarPromociones: vi.fn().mockResolvedValue({ items: [] }),
    crearPromocion: vi.fn().mockResolvedValue({ id: 'p1' }),
    obtenerPromocion: vi.fn().mockResolvedValue({ id: 'p1' }),
    actualizarPromocion: vi.fn().mockResolvedValue({ id: 'p1' }),
    eliminarPromocion: vi.fn().mockResolvedValue(undefined),
    listarMovimientos: vi.fn().mockResolvedValue({ items: [] }),
    registrarMovimiento: vi.fn().mockResolvedValue({ id: 'm1' }),
  };
  return { controlador: new OperacionesController(servicio as never), servicio };
}

const empresario = {
  user_id: 'u1',
  email: 'e@test.dev',
  rol: ROLES.EMPRESARIO,
  tenant_id: EMPRESA,
};
const admin = { user_id: 'a1', email: 'a@test.dev', rol: ROLES.ADMIN };

describe('OperacionesController (R-PS-04)', () => {
  it('delega todas las operaciones con el tenant del empresario', async () => {
    const { controlador, servicio } = crear();
    await controlador.configuracion(undefined, empresario);
    await controlador.guardarConfiguracion({}, undefined, empresario);
    await controlador.importar({ filas: [] } as never, undefined, empresario);
    await controlador.escanear('750', undefined, empresario);
    await controlador.listarCombos({} as never, empresario);
    await controlador.crearCombo({ sku: 'C-1' } as never, undefined, empresario);
    await controlador.detalleCombo('c1', undefined, empresario);
    await controlador.actualizarCombo('c1', { nombre: 'Nuevo' } as never, undefined, empresario);
    await controlador.eliminarCombo('c1', undefined, empresario);
    await controlador.listarPromociones({} as never, empresario);
    await controlador.crearPromocion({ nombre: 'Promo' } as never, undefined, empresario);
    await controlador.detallePromocion('p1', undefined, empresario);
    await controlador.actualizarPromocion('p1', { nombre: 'Nueva' } as never, undefined, empresario);
    await controlador.eliminarPromocion('p1', undefined, empresario);
    await controlador.listarMovimientos({} as never, empresario);
    await controlador.registrarMovimiento(
      { tipo: 'entrada', lineas: [] } as never,
      undefined,
      empresario,
    );

    for (const fn of Object.values(servicio)) {
      expect(fn).toHaveBeenCalled();
    }
    expect(servicio.obtenerConfiguracion).toHaveBeenCalledWith(EMPRESA);
    expect(servicio.registrarMovimiento).toHaveBeenCalledWith(
      EMPRESA,
      expect.any(Object),
      'u1',
    );
  });

  it('permite admin con empresa explícita y exige empresa sin ella', async () => {
    const { controlador, servicio } = crear();
    await controlador.configuracion(EMPRESA, admin);
    expect(servicio.obtenerConfiguracion).toHaveBeenCalledWith(EMPRESA);

    await expect(async () => controlador.configuracion(undefined, admin)).rejects.toMatchObject({
      codigo: 'EMPRESA_REQUERIDA',
    });
  });

  it('rechaza empresa distinta para el empresario y tenant ausente', async () => {
    const { controlador } = crear();
    await expect(async () =>
      controlador.configuracion('otra', empresario),
    ).rejects.toMatchObject({ codigo: 'EMPRESA_NO_AUTORIZADA' });
    await expect(async () =>
      controlador.configuracion(undefined, { ...empresario, tenant_id: undefined }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });

  it('convierte detalle inexistente en 404', async () => {
    const { controlador, servicio } = crear();
    servicio.obtenerCombo.mockResolvedValueOnce(null);
    await expect(controlador.detalleCombo('c1', undefined, empresario)).rejects.toBeInstanceOf(
      NotFoundError,
    );
    servicio.obtenerPromocion.mockResolvedValueOnce(null);
    await expect(controlador.detallePromocion('p1', undefined, empresario)).rejects.toBeInstanceOf(
      NotFoundError,
    );
  });

  it('propaga DomainError del servicio', async () => {
    const { controlador, servicio } = crear();
    servicio.crearCombo.mockRejectedValueOnce(new DomainError('X', 'falló'));
    await expect(
      controlador.crearCombo({ sku: 'C-1' } as never, undefined, empresario),
    ).rejects.toBeInstanceOf(DomainError);
  });
});
