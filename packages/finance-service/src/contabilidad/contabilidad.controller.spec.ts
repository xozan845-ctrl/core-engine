import 'reflect-metadata';
import { ContabilidadController } from './contabilidad.controller';
import type { ContabilidadService } from './contabilidad.service';
import { NotFoundError, ROLES } from '@core/shared';

const asiento = { id: 'a-1', concepto: 'Venta', tipo: 'INGRESO', total_debe_cents: 1000, total_haber_cents: 1000 };
const cuenta = { codigo: '1101', nombre: 'Caja', tipo: 'ACTIVO', naturaleza: 'DEUDORA' };
const admin = { user_id: 'u-a', email: 'a@core.local', rol: ROLES.ADMIN };

function fabricar(): { contabilidad: Record<string, jest.Mock>; controller: ContabilidadController } {
  const contabilidad = {
    planDeCuentas: jest.fn(),
    crearCuenta: jest.fn(),
    estadoCuenta: jest.fn(),
    libroDiario: jest.fn(),
    registrar: jest.fn(),
    anular: jest.fn(),
    libroMayor: jest.fn(),
    libroVentas: jest.fn(),
    libroCompras: jest.fn(),
  };
  const controller = new ContabilidadController(contabilidad as unknown as ContabilidadService);
  return { contabilidad, controller };
}

describe('ContabilidadController (api/v1/finanzas, partida doble, R-U-10/11)', () => {
  it('debe devolver el plan de cuentas', async () => {
    const { contabilidad, controller } = fabricar();
    contabilidad.planDeCuentas.mockResolvedValue([cuenta]);
    await expect(controller.cuentas()).resolves.toEqual([cuenta]);
  });

  it('debe crear una cuenta con el dto', async () => {
    const { contabilidad, controller } = fabricar();
    contabilidad.crearCuenta.mockResolvedValue(cuenta);
    const dto = { codigo: '1101', nombre: 'Caja', tipo: 'ACTIVO', naturaleza: 'DEUDORA' };
    await expect(controller.crearCuenta(dto as never)).resolves.toEqual(cuenta);
    expect(contabilidad.crearCuenta).toHaveBeenCalledWith(dto);
  });

  it('debe cambiar el estado de la cuenta cuando existe y lanzar NotFoundError cuando no', async () => {
    const { contabilidad, controller } = fabricar();
    contabilidad.estadoCuenta.mockResolvedValue(cuenta);
    await expect(controller.estadoCuenta('1101', { estado: 'inactiva' } as never)).resolves.toEqual(cuenta);
    expect(contabilidad.estadoCuenta).toHaveBeenCalledWith('1101', 'inactiva');
    contabilidad.estadoCuenta.mockResolvedValue(null);
    await expect(controller.estadoCuenta('9999', { estado: 'inactiva' } as never)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe listar el libro diario con limite numerico por defecto 200', async () => {
    const { contabilidad, controller } = fabricar();
    contabilidad.libroDiario.mockResolvedValue([asiento]);
    await controller.libroDiario('2026-09-01', '2026-09-30', undefined);
    expect(contabilidad.libroDiario).toHaveBeenCalledWith('2026-09-01', '2026-09-30', 200);
    await controller.libroDiario(undefined, undefined, '50');
    expect(contabilidad.libroDiario).toHaveBeenCalledWith(undefined, undefined, 50);
  });

  it('debe registrar un asiento construyendo los parametros con el usuario y el tipo por defecto', async () => {
    const { contabilidad, controller } = fabricar();
    contabilidad.registrar.mockResolvedValue(asiento);
    const dto = {
      concepto: 'Venta manual',
      detalles: [{ cuenta_codigo: '1101', debe_cents: 1000, haber_cents: 0 }],
    };
    await expect(controller.registrar(dto as never, admin as never)).resolves.toEqual(asiento);
    expect(contabilidad.registrar).toHaveBeenCalledWith({
      concepto: 'Venta manual',
      tipo: 'MANUAL',
      fecha: undefined,
      referencia_tipo: undefined,
      referencia_id: undefined,
      creado_por: 'u-a',
      detalles: dto.detalles,
    });
  });

  it('debe anular el asiento cuando existe y lanzar NotFoundError cuando no', async () => {
    const { contabilidad, controller } = fabricar();
    contabilidad.anular.mockResolvedValue(asiento);
    await expect(controller.anular('a-1')).resolves.toEqual(asiento);
    contabilidad.anular.mockResolvedValue(null);
    await expect(controller.anular('a-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe delegar libro mayor, ventas y compras', async () => {
    const { contabilidad, controller } = fabricar();
    contabilidad.libroMayor.mockResolvedValue([]);
    contabilidad.libroVentas.mockResolvedValue([]);
    contabilidad.libroCompras.mockResolvedValue([]);
    await controller.libroMayor('1101', '2026-09-01', '2026-09-30');
    await controller.libroVentas('2026-09-01', '2026-09-30');
    await controller.libroCompras('2026-09-01', '2026-09-30');
    expect(contabilidad.libroMayor).toHaveBeenCalledWith('1101', '2026-09-01', '2026-09-30');
    expect(contabilidad.libroVentas).toHaveBeenCalledWith('2026-09-01', '2026-09-30');
    expect(contabilidad.libroCompras).toHaveBeenCalledWith('2026-09-01', '2026-09-30');
  });

  it('debe exigir rol admin a nivel de controlador', () => {
    expect(Reflect.getMetadata('roles_requeridos', ContabilidadController)).toEqual([ROLES.ADMIN]);
  });
});