import { ROLES } from '@core/shared';
import { SeedService } from './seed.service';

describe('SeedService', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it('no crea otro administrador cuando ya existe', async () => {
    const usuarios = { existeAdmin: vi.fn().mockResolvedValue(true), crear: vi.fn() };
    const pg = { queryOne: vi.fn().mockResolvedValue(null), query: vi.fn().mockResolvedValue([]) };
    const servicio = new SeedService(usuarios as never, pg as never);
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
    const pg = { queryOne: vi.fn().mockResolvedValue(null), query: vi.fn().mockResolvedValue([]) };
    const servicio = new SeedService(usuarios as never, pg as never);
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
    const pg = { queryOne: vi.fn().mockResolvedValue(null), query: vi.fn().mockResolvedValue([]) };
    const servicio = new SeedService(usuarios as never, pg as never);
    await expect(servicio.onApplicationBootstrap()).resolves.toBeUndefined();
  });

  it('tolera errores no Error durante la siembra', async () => {
    const usuarios = {
      existeAdmin: vi.fn().mockResolvedValue(false),
      crear: vi.fn().mockRejectedValue('fallo'),
    };
    const pg = { queryOne: vi.fn().mockResolvedValue(null), query: vi.fn().mockResolvedValue([]) };
    const servicio = new SeedService(usuarios as never, pg as never);
    await expect(servicio.onApplicationBootstrap()).resolves.toBeUndefined();
  });
});

  it('asegura el tenant de plataforma con las capacidades del pack flota', async () => {
    const usuarios = { existeAdmin: vi.fn().mockResolvedValue(true), crear: vi.fn() };
    const pg = {
      queryOne: vi.fn().mockResolvedValue({ id: 'admin-1' }),
      query: vi.fn().mockResolvedValue([]),
    };
    const servicio = new SeedService(usuarios as never, pg as never);
    await servicio.onApplicationBootstrap();
    expect(pg.query).toHaveBeenCalledTimes(2);
    expect(pg.query.mock.calls[0][0]).toContain('INSERT INTO identity.empresas');
    expect(pg.query.mock.calls[1][0]).toContain('identity.pack_capacidades');
    expect(pg.query.mock.calls[0][1][0]).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
    );
  });
