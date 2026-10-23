import 'reflect-metadata';
import { BucketsController } from './buckets.controller';
import { ROLES, UsuarioContexto } from '@core/shared';
import type { BucketsService } from './buckets.service';

const usuario: UsuarioContexto = {
  user_id: 'admin-1',
  email: 'admin@core-engine.test',
  rol: ROLES.ADMIN,
  tenant_id: '11111111-1111-4111-8111-111111111111',
};

const crear = () => {
  const buckets = {
    asegurar: vi.fn().mockResolvedValue({ bucket: 't-1', cuota_bytes: 1000 }),
    actualizarCuota: vi.fn().mockResolvedValue({ bucket: 't-1', cuota_bytes: 2000 }),
  };
  return { controller: new BucketsController(buckets as unknown as BucketsService), buckets };
};

describe('BucketsController (api/v1/storage/bucket)', () => {
  it('debe devolver el estado del bucket creandolo de forma perezosa', async () => {
    const { controller, buckets } = crear();

    const estado = await controller.estado(usuario);

    expect(buckets.asegurar).toHaveBeenCalledWith(usuario.tenant_id);
    expect(estado).toEqual({ bucket: 't-1', cuota_bytes: 1000 });
  });

  it('debe asegurar el bucket sin tocar la cuota cuando no se envia', async () => {
    const { controller, buckets } = crear();

    await controller.configurar({}, usuario);

    expect(buckets.asegurar).toHaveBeenCalledWith(usuario.tenant_id);
    expect(buckets.actualizarCuota).not.toHaveBeenCalled();
  });

  it('debe actualizar la cuota cuando se envia (admin)', async () => {
    const { controller, buckets } = crear();

    const estado = await controller.configurar({ cuota_bytes: 2000 }, usuario);

    expect(buckets.actualizarCuota).toHaveBeenCalledWith(usuario.tenant_id, 2000);
    expect(estado).toEqual({ bucket: 't-1', cuota_bytes: 2000 });
  });

  it('debe restringir la configuracion del bucket a los roles globales', () => {
    expect(Reflect.getMetadata('roles_requeridos', BucketsController.prototype.configurar)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
    ]);
  });
});
