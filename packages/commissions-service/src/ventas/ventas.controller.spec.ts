import type { Mock } from 'vitest';
import 'reflect-metadata';
import { VentasController, VentasEmpresaController } from './ventas.controller';
import type { VentasService } from './ventas.service';
import { NotFoundError, ROLES } from '@core/shared';

const estado = { total_ventas_cents: 115000, comisiones_cents: 13800, liquidado_cents: 0 };
const estadoEmpresa = { empresa_id: 'empresa-1', ventas: [], comisiones_pendientes: '0.00', liquidaciones: [] };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };
const empresario = { user_id: 'u-e', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 'empresa-1' };

function fabricar(): { ventas: Record<string, Mock>; controller: VentasController } {
  const ventas = { estadoCuenta: vi.fn() };
  const controller = new VentasController(ventas as unknown as VentasService);
  return { ventas, controller };
}

describe('VentasController (api/v1/vendedores/me/ventas, R-U-10/11)', () => {
  it('debe devolver el estado de cuenta del vendedor autenticado', async () => {
    const { ventas, controller } = fabricar();
    ventas.estadoCuenta.mockResolvedValue(estado);
    await expect(controller.estado(vendedor as never)).resolves.toEqual(estado);
    expect(ventas.estadoCuenta).toHaveBeenCalledWith('u-v');
  });

  it('debe exigir rol vendedor', () => {
    expect(Reflect.getMetadata('roles_requeridos', VentasController.prototype.estado)).toEqual([ROLES.VENDEDOR]);
  });
});

describe('VentasEmpresaController (api/v1/empresarios/me/ventas, ADR-25)', () => {
  function fabricar(): { ventas: Record<string, Mock>; controller: VentasEmpresaController } {
    const ventas = { estadoCuentaEmpresa: vi.fn() };
    const controller = new VentasEmpresaController(ventas as unknown as VentasService);
    return { ventas, controller };
  }

  it('debe devolver el estado de cuenta de la empresa activa', async () => {
    const { ventas, controller } = fabricar();
    ventas.estadoCuentaEmpresa.mockResolvedValue(estadoEmpresa);
    await expect(controller.estado(empresario as never)).resolves.toEqual(estadoEmpresa);
    expect(ventas.estadoCuentaEmpresa).toHaveBeenCalledWith('empresa-1');
  });

  it('debe rechazar al empresario sin empresa activa', async () => {
    const { ventas, controller } = fabricar();
    await expect(
      controller.estado({ ...empresario, tenant_id: undefined } as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    expect(ventas.estadoCuentaEmpresa).not.toHaveBeenCalled();
  });

  it('debe exigir rol empresario', () => {
    expect(Reflect.getMetadata('roles_requeridos', VentasEmpresaController.prototype.estado)).toEqual([
      ROLES.EMPRESARIO,
    ]);
  });
});