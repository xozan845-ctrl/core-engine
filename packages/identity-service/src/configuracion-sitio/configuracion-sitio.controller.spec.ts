import type { Mock } from 'vitest';
import 'reflect-metadata';
import { ROLES } from '@core/shared';
import { ConfiguracionSitioController } from './configuracion-sitio.controller';
import type { ConfiguracionSitioService } from './configuracion-sitio.service';

describe('ConfiguracionSitioController (ADR-38)', () => {
  const configuracion = {
    obtener: vi.fn(),
    actualizar: vi.fn(),
  };
  const controller = new ConfiguracionSitioController(
    configuracion as unknown as ConfiguracionSitioService,
  );
  const admin = { user_id: 'admin-1', email: 'admin@core.test', rol: ROLES.ADMIN };

  beforeEach(() => vi.clearAllMocks());

  it('sirve el fondo público con cache de 60 segundos', async () => {
    const fondo = { background_objeto_id: null, background_url: null };
    configuracion.obtener.mockResolvedValue(fondo);
    const res = { setHeader: vi.fn() } as unknown as { setHeader: Mock };

    await expect(controller.obtener(res as never)).resolves.toEqual(fondo);

    expect(res.setHeader).toHaveBeenCalledWith('Cache-Control', 'public, max-age=60');
    expect(configuracion.obtener).toHaveBeenCalled();
  });

  it('actualiza el fondo pasando el usuario autenticado', async () => {
    const dto = { background_objeto_id: '11111111-2222-4333-8444-555555555555' };
    const actualizado = { background_objeto_id: dto.background_objeto_id };
    configuracion.actualizar.mockResolvedValue(actualizado);

    await expect(controller.actualizar(dto, admin)).resolves.toEqual(actualizado);
    expect(configuracion.actualizar).toHaveBeenCalledWith(dto, 'admin-1');
  });

  it('restringe el PATCH a admin y super_admin', () => {
    expect(
      Reflect.getMetadata('roles_requeridos', ConfiguracionSitioController.prototype.actualizar),
    ).toEqual([ROLES.SUPER_ADMIN, ROLES.ADMIN]);
    expect(
      Reflect.getMetadata('roles_requeridos', ConfiguracionSitioController.prototype.obtener),
    ).toBeUndefined();
  });
});
