import { ROLES } from '@core/shared';
import { SeedService } from './seed.service';

describe('SeedService', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it('no crea otro administrador cuando ya existe', async () => {
    const usuarios = { existeAdmin: vi.fn().mockResolvedValue(true), crear: vi.fn() };
    const servicio = new SeedService(usuarios as never);
    await servicio.onApplicationBootstrap();
    expect(usuarios.crear).not.toHaveBeenCalled();
  });

  it('crea el super admin inicial con credenciales del entorno', async () => {
    vi.stubEnv('ADMIN_EMAIL', 'super@core.test');
    vi.stubEnv('ADMIN_PASSWORD', 'clave-segura');
    const usuarios = {
      existeAdmin: vi.fn().mockResolvedValue(false),
      crear: vi.fn().mockResolvedValue({ correo: 'super@core.test' }),
    };
    const servicio = new SeedService(usuarios as never);
    await servicio.onApplicationBootstrap();
    expect(usuarios.crear).toHaveBeenCalledWith({
      nombre: 'Administrador Core Engine',
      correo: 'super@core.test',
      contrasena: 'clave-segura',
      rol: ROLES.SUPER_ADMIN,
    });
  });

  it('tolera un error durante la siembra para no detener el arranque', async () => {
    const usuarios = {
      existeAdmin: vi.fn().mockResolvedValue(false),
      crear: vi.fn().mockRejectedValue(new Error('base no disponible')),
    };
    const servicio = new SeedService(usuarios as never);
    await expect(servicio.onApplicationBootstrap()).resolves.toBeUndefined();
  });

  it('tolera errores no Error durante la siembra', async () => {
    const usuarios = {
      existeAdmin: vi.fn().mockResolvedValue(false),
      crear: vi.fn().mockRejectedValue('fallo'),
    };
    const servicio = new SeedService(usuarios as never);
    await expect(servicio.onApplicationBootstrap()).resolves.toBeUndefined();
  });
});
