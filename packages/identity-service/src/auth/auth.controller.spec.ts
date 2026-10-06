import 'reflect-metadata';
import { AuthController } from './auth.controller';
import type { AuthService } from './auth.service';
import type { UsuariosService } from '../usuarios/usuarios.service';
import { ROLES, DomainError, UnauthorizedError, ForbiddenError } from '@core/shared';

const sesion = {
  access_token: 'access-1',
  refresh_token: 'refresh-1',
  expira_en: 900,
  usuario: { id: 'u-1', nombre: 'Ana', correo: 'ana@core.local', rol: ROLES.VENDEDOR },
};

const adminContexto = { user_id: 'u-admin', email: 'admin@core.local', rol: ROLES.ADMIN };

function fabricar(): {
  auth: Record<string, jest.Mock>;
  usuarios: Record<string, jest.Mock>;
  controller: AuthController;
} {
  const auth = { registrar: jest.fn(), login: jest.fn(), refrescar: jest.fn(), cerrarSesion: jest.fn().mockResolvedValue(1) };
  const usuarios = {
    crear: jest.fn(),
    cambiarContrasena: jest.fn(),
    vincularPersonal: jest.fn(),
    actualizarPerfil: jest.fn(),
    encontrarPorCorreo: jest.fn(),
  };
  const controller = new AuthController(
    auth as unknown as AuthService,
    usuarios as unknown as UsuariosService,
  );
  return { auth, usuarios, controller };
}

describe('AuthController (api/v1/auth, R-U-10/11)', () => {
  it('debe registrar con el dto completo y devolver la sesion', async () => {
    const { auth, controller } = fabricar();
    auth.registrar.mockResolvedValue(sesion);
    const dto = { nombre: 'Ana', correo: 'ana@core.local', contrasena: 'secreta123', rol: ROLES.VENDEDOR };
    await expect(controller.registrar(dto as never)).resolves.toEqual(sesion);
    expect(auth.registrar).toHaveBeenCalledWith(dto);
  });

  it('debe hacer login con correo y contrasena separados', async () => {
    const { auth, controller } = fabricar();
    auth.login.mockResolvedValue(sesion);
    await expect(controller.login({ correo: 'ana@core.local', contrasena: 'secreta123' } as never)).resolves.toEqual(sesion);
    expect(auth.login).toHaveBeenCalledWith('ana@core.local', 'secreta123');
  });

  it('debe refrescar la sesion cuando llega el refresh_token', async () => {
    const { auth, controller } = fabricar();
    auth.refrescar.mockResolvedValue(sesion);
    await expect(controller.refresh({ refresh_token: 'refresh-1' })).resolves.toEqual(sesion);
    expect(auth.refrescar).toHaveBeenCalledWith('refresh-1');
  });

  it('debe lanzar DomainError TOKEN_FALTANTE cuando el refresh no trae token', async () => {
    const { controller } = fabricar();
    let error: DomainError | undefined;
    try {
      await controller.refresh({ refresh_token: '' });
    } catch (e) {
      error = e as DomainError;
    }
    expect(error?.codigo).toBe('TOKEN_FALTANTE');
  });

  it('debe crear el usuario admin construyendo los datos al servicio', async () => {
    const { usuarios, controller } = fabricar();
    const usuario = { id: 'u-2', nombre: 'Luis', correo: 'luis@core.local', rol: ROLES.VENDEDOR };
    usuarios.crear.mockResolvedValue(usuario);
    const dto = { nombre: 'Luis', correo: 'luis@core.local', contrasena: 'secreta123', rol: ROLES.VENDEDOR };
    await expect(controller.crearUsuario(dto as never, adminContexto as never)).resolves.toEqual(usuario);
    expect(usuarios.crear).toHaveBeenCalledWith({
      nombre: 'Luis',
      correo: 'luis@core.local',
      contrasena: 'secreta123',
      rol: ROLES.VENDEDOR,
      tenant_id: undefined,
      personal_id: undefined,
    });
  });

  it('debe rechazar crear-usuario cuando falta contexto o no es admin', async () => {
    const { controller } = fabricar();
    const dto = { nombre: 'L', correo: 'l@core.local', contrasena: 'secreta123', rol: ROLES.VENDEDOR };
    await expect(controller.crearUsuario(dto as never, undefined as never)).rejects.toBeInstanceOf(UnauthorizedError);
    await expect(
      controller.crearUsuario(dto as never, { user_id: 'u-1', email: 'v@c', rol: ROLES.VENDEDOR } as never),
    ).rejects.toBeInstanceOf(ForbiddenError);
  });

  it('debe exigir tenant_id para los roles de logistica en crear-usuario', async () => {
    const { controller } = fabricar();
    const dto = { nombre: 'Oper', correo: 'op@core.local', contrasena: 'secreta123', rol: ROLES.LOGISTICA };
    let error: DomainError | undefined;
    try {
      await controller.crearUsuario(dto as never, adminContexto as never);
    } catch (e) {
      error = e as DomainError;
    }
    expect(error?.codigo).toBe('TENANT_REQUERIDO');
  });

  it('debe cambiar la contrasena del usuario autenticado', async () => {
    const { auth, usuarios, controller } = fabricar();
    await expect(
      controller.cambiarContrasena(
        { actual: 'vieja', nueva: 'nueva123' } as never,
        adminContexto as never,
      ),
    ).resolves.toEqual({ ok: true });
    expect(usuarios.cambiarContrasena).toHaveBeenCalledWith('u-admin', 'vieja', 'nueva123');
    expect(auth.cerrarSesion).toHaveBeenCalledWith('u-admin');
  });

  it('debe rechazar cambiar-contrasena cuando falta el contexto', async () => {
    const { controller } = fabricar();
    await expect(controller.cambiarContrasena({ actual: 'a', nueva: 'b' } as never, undefined as never)).rejects.toBeInstanceOf(UnauthorizedError);
  });

  it('debe responder ok en restablecer-contrasena aunque el correo no exista', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.encontrarPorCorreo.mockResolvedValue(null);
    await expect(controller.restablecerContrasena({ correo: 'nadie@core.local' })).resolves.toEqual({ ok: true });
    expect(usuarios.encontrarPorCorreo).toHaveBeenCalledWith('nadie@core.local');
  });

  it('debe devolver el contexto desde las cabeceras en /me', async () => {
    const { controller } = fabricar();
    const contexto = await controller.yo({
      'x-user-id': 'u-1',
      'x-user-email': 'ana@core.local',
      'x-user-rol': ROLES.VENDEDOR,
    });
    expect(contexto?.user_id).toBe('u-1');
    expect(contexto?.rol).toBe(ROLES.VENDEDOR);
  });

  it('debe rechazar /me cuando falta el contexto', async () => {
    const { controller } = fabricar();
    await expect(controller.yo({})).rejects.toBeInstanceOf(UnauthorizedError);
  });

  it('debe cerrar solo la sesion indicada cuando el logout trae refresh_token', async () => {
    const { auth, controller } = fabricar();
    await expect(
      controller.logout({ refresh_token: 'refresh-1' } as never, adminContexto as never),
    ).resolves.toEqual({ ok: true });
    expect(auth.cerrarSesion).toHaveBeenCalledWith('u-admin', 'refresh-1');
  });

  it('debe cerrar todas las sesiones cuando el logout no trae refresh_token', async () => {
    const { auth, controller } = fabricar();
    await expect(controller.logout({} as never, adminContexto as never)).resolves.toEqual({ ok: true });
    expect(auth.cerrarSesion).toHaveBeenCalledWith('u-admin', undefined);
  });

  it('debe rechazar el logout cuando falta el contexto', async () => {
    const { controller } = fabricar();
    await expect(controller.logout({} as never, undefined as never)).rejects.toBeInstanceOf(UnauthorizedError);
  });

  it('debe vincular la ficha de personal y actualizar el nombre si viene', async () => {
    const { usuarios, controller } = fabricar();
    await expect(
      controller.vincularPersonal({ personal_id: 'ficha-1', nombre: 'Ana P' } as never, adminContexto as never),
    ).resolves.toEqual({ ok: true });
    expect(usuarios.vincularPersonal).toHaveBeenCalledWith('u-admin', 'ficha-1');
    expect(usuarios.actualizarPerfil).toHaveBeenCalledWith('u-admin', { nombre: 'Ana P' });
  });
});