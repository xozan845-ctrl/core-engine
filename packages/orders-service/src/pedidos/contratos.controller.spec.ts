import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CAPABILITIES_KEY, CAPACIDADES, ROLES } from '@core/shared';
import { ContratosController } from './contratos.controller';
import type { ContratosService } from './contratos.service';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };
const CONTRATO = '22222222-2222-4222-8222-222222222222';
const ORDEN = '33333333-3333-4333-8333-333333333333';

function fabricar(): { servicio: Record<string, Mock>; controller: ContratosController } {
  const servicio = {
    listar: vi.fn(),
    obtener: vi.fn(),
    crear: vi.fn(),
    actualizar: vi.fn(),
    finalizar: vi.fn(),
    listarEventos: vi.fn(),
  };
  return { servicio, controller: new ContratosController(servicio as unknown as ContratosService) };
}

describe('ContratosController', () => {
  it('exige la capacidad de contratos y roles administrativos', () => {
    expect(Reflect.getMetadata(CAPABILITIES_KEY, ContratosController)).toEqual([
      CAPACIDADES.CONTRATOS,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', ContratosController)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
      ROLES.EMPRESARIO,
    ]);
  });

  it('resuelve la empresa del empresario y exige empresa_id al admin', async () => {
    const { servicio, controller } = fabricar();
    servicio.listar.mockResolvedValue([]);
    await expect(
      controller.listar(undefined, undefined, undefined, undefined, undefined, undefined, empresario as never),
    ).resolves.toEqual([]);
    expect(servicio.listar).toHaveBeenCalledWith('t-1', {
      estado: undefined,
      order_id: undefined,
      parte_id: undefined,
      pagina: undefined,
      limite: undefined,
    });
    expect(() =>
      controller.listar(undefined, undefined, undefined, undefined, undefined, undefined, admin as never),
    ).toThrow(/empresa_id/);
  });

  it('delega alta, consulta, actualizacion y finalizacion', async () => {
    const { servicio, controller } = fabricar();
    servicio.crear.mockResolvedValue({ id: CONTRATO });
    servicio.obtener.mockResolvedValue({ id: CONTRATO });
    servicio.actualizar.mockResolvedValue({ id: CONTRATO });
    servicio.finalizar.mockResolvedValue({ id: CONTRATO });
    servicio.listarEventos.mockResolvedValue([]);

    await controller.crear(
      { titulo: 'Alquiler', deposito: '500.00' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crear).toHaveBeenCalledWith('t-1', 'u-1', {
      titulo: 'Alquiler',
      deposito: '500.00',
    });

    await controller.obtener(CONTRATO, undefined, empresario as never);
    expect(servicio.obtener).toHaveBeenCalledWith('t-1', CONTRATO);

    await controller.actualizar(
      CONTRATO,
      { estado: 'activo' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.actualizar).toHaveBeenCalledWith('t-1', CONTRATO, { estado: 'activo' });

    await controller.finalizar(
      CONTRATO,
      { retenido: '100.00', motivo: 'danos' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.finalizar).toHaveBeenCalledWith('t-1', CONTRATO, 'u-1', {
      retenido: '100.00',
      motivo: 'danos',
    });

    await controller.movimientos(CONTRATO, undefined, empresario as never);
    expect(servicio.listarEventos).toHaveBeenCalledWith('t-1', CONTRATO);
  });

  it('permite al admin indicar la empresa y rechaza a un empresario ajeno', async () => {
    const { servicio, controller } = fabricar();
    servicio.obtener.mockResolvedValue({ id: CONTRATO });
    await controller.obtener(CONTRATO, 'e-9', admin as never);
    expect(servicio.obtener).toHaveBeenCalledWith('e-9', CONTRATO);
    expect(() => controller.obtener(CONTRATO, 'otra', empresario as never)).toThrow(
      /No puedes operar otra empresa/,
    );
    expect(() =>
      controller.obtener(ORDEN, undefined, { ...empresario, tenant_id: undefined } as never),
    ).toThrow(/No hay empresa activa/);
  });
});
