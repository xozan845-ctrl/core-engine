import 'reflect-metadata';
import { AdminStorageController } from './admin.controller';
import { ROLES } from '@core/shared';
import type { BucketsService } from '../buckets/buckets.service';

const crear = () => {
  const buckets = {
    listar: vi.fn().mockResolvedValue({ items: [], total: 0 }),
    estado: vi.fn().mockResolvedValue({ tenant_id: 't1', cuota_bytes: 1000 }),
    actualizarCuota: vi.fn().mockResolvedValue({ tenant_id: 't1', cuota_bytes: 3000 }),
    cambiarEstado: vi.fn().mockResolvedValue({ tenant_id: 't1', estado: 'suspendido' }),
    recalcularUso: vi.fn().mockResolvedValue({ tenant_id: 't1', usado_bytes: 500 }),
  };
  return { controller: new AdminStorageController(buckets as unknown as BucketsService), buckets };
};

describe('AdminStorageController (api/v1/admin/storage, multi-tenant)', () => {
  it('debe listar los tenants con almacenamiento', async () => {
    const { controller, buckets } = crear();

    await controller.listar({ pagina: 1, limite: 20 });

    expect(buckets.listar).toHaveBeenCalledWith(1, 20);
  });

  it('debe devolver el estado de un tenant', async () => {
    const { controller, buckets } = crear();

    await controller.estado('t1');

    expect(buckets.estado).toHaveBeenCalledWith('t1');
  });

  it('debe aplicar cuota y estado cuando se envian', async () => {
    const { controller, buckets } = crear();

    await controller.actualizar('t1', { cuota_bytes: 3000, estado: 'suspendido' });

    expect(buckets.actualizarCuota).toHaveBeenCalledWith('t1', 3000);
    expect(buckets.cambiarEstado).toHaveBeenCalledWith('t1', 'suspendido');
    expect(buckets.estado).toHaveBeenCalledWith('t1');
  });

  it('no debe tocar cuota ni estado cuando el DTO viene vacio', async () => {
    const { controller, buckets } = crear();

    await controller.actualizar('t1', {});

    expect(buckets.actualizarCuota).not.toHaveBeenCalled();
    expect(buckets.cambiarEstado).not.toHaveBeenCalled();
  });

  it('debe recalcular el uso de un tenant', async () => {
    const { controller, buckets } = crear();

    await controller.recalcular('t1');

    expect(buckets.recalcularUso).toHaveBeenCalledWith('t1');
  });

  it('debe restringir el controlador al rol admin', () => {
    expect(Reflect.getMetadata('roles_requeridos', AdminStorageController)).toEqual([ROLES.SUPER_ADMIN, ROLES.ADMIN]);
  });
});
