import 'reflect-metadata';
import { InternalStorageController } from './internal.controller';
import { ClaveInternaGuard } from '@core/shared';

const crear = () => {
  const buckets = {
    estado: vi.fn().mockResolvedValue({ tenant_id: 't1' }),
    recalcularUso: vi.fn().mockResolvedValue({ tenant_id: 't1', usado_bytes: 1 }),
  };
  const objetos = { obtener: vi.fn().mockResolvedValue({ id: 'o1' }) };
  return {
    controller: new InternalStorageController(buckets as never, objetos as never),
    buckets,
    objetos,
  };
};

describe('InternalStorageController (internal/storage, servicio→servicio)', () => {
  it('debe proteger los endpoints internos con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternalStorageController) ?? [];
    expect(guards).toContain(ClaveInternaGuard);
  });

  it('debe devolver el estado del tenant', async () => {
    const { controller, buckets } = crear();
    await controller.estado('t1');
    expect(buckets.estado).toHaveBeenCalledWith('t1');
  });

  it('debe recalcular el uso del tenant', async () => {
    const { controller, buckets } = crear();
    await controller.recalcular('t1');
    expect(buckets.recalcularUso).toHaveBeenCalledWith('t1');
  });

  it('debe resolver la metadata de un objeto cruando el tenant', async () => {
    const { controller, objetos } = crear();
    await controller.objeto('o1', 't1');
    expect(objetos.obtener).toHaveBeenCalledWith('t1', 'o1');
  });
});
