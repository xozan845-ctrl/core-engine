import { AuthService } from './auth.service';
import { UsuariosService, Usuario } from '../usuarios/usuarios.service';
import { DomainError, ROLES, crearRefreshToken, crearAccessToken } from '@core/shared';

/**
 * Registro, login y refresh (doc 4.3): JWT de corta duracion + refresh.
 * El registro publico solo admite vendedor/comprador (RF-01).
 */
describe('AuthService (sesiones JWT)', () => {
  const SECRETO = process.env.JWT_SECRET ?? 'dev_secret';

  const usuarioBase: Usuario = {
    id: 'u1',
    nombre: 'Ana',
    correo: 'ana@core.test',
    rol: 'vendedor',
    creado_en: '2026-09-01T00:00:00.000Z',
  };

  const crear = () => {
    const usuarios = {
      crear: jest.fn().mockResolvedValue(usuarioBase),
      verificarCredenciales: jest.fn().mockResolvedValue(usuarioBase),
      encontrarPorId: jest.fn().mockResolvedValue(usuarioBase),
    } as unknown as UsuariosService;
    const servicio = new AuthService(usuarios);
    return { servicio, usuarios };
  };

  const contexto = { id: 'u1', email: 'ana@core.test', rol: 'vendedor' as const, nombre: 'Ana' };

  describe('registrar', () => {
    it('debe rechazar el rol admin cuando el registro es publico', async () => {
      const { servicio, usuarios } = crear();
      await expect(
        servicio.registrar({ nombre: 'Ana', correo: 'a@t.co', contrasena: 'secreta123', rol: ROLES.ADMIN }),
      ).rejects.toThrow(DomainError);
      expect(usuarios.crear).not.toHaveBeenCalled();
    });

    it('debe crear la sesion con access y refresh cuando el rol es vendedor', async () => {
      const { servicio } = crear();
      const sesion = await servicio.registrar({
        nombre: 'Ana',
        correo: 'ana@core.test',
        contrasena: 'secreta123',
        rol: ROLES.VENDEDOR,
      });

      expect(sesion.access_token.split('.')).toHaveLength(3); // JWT HS256
      expect(sesion.refresh_token.split('.')).toHaveLength(3);
      expect(sesion.usuario).toEqual({ id: 'u1', nombre: 'Ana', correo: 'ana@core.test', rol: 'vendedor' });
      expect(sesion.expira_en).toBe(900);
    });
  });

  describe('login', () => {
    it('debe devolver la sesion cuando las credenciales son validas', async () => {
      const { servicio, usuarios } = crear();
      const sesion = await servicio.login('ana@core.test', 'secreta123');
      expect(usuarios.verificarCredenciales).toHaveBeenCalledWith('ana@core.test', 'secreta123');
      expect(sesion.usuario.id).toBe('u1');
    });

    it('debe propagar el UnauthorizedError cuando las credenciales son invalidas', async () => {
      const { servicio, usuarios } = crear();
      (usuarios.verificarCredenciales as jest.Mock).mockRejectedValue(
        new Error('Correo o contrasena incorrectos.'),
      );
      await expect(servicio.login('ana@core.test', 'mala')).rejects.toThrow(/incorrectos/);
    });
  });

  describe('refrescar', () => {
    it('debe rechazar el token cuando no es un JWT valido', async () => {
      const { servicio } = crear();
      await expect(servicio.refrescar('token-basura')).rejects.toThrow(/invalido|expiro/);
    });

    it('debe rechazar el token cuando no es de tipo refresh', async () => {
      const { servicio } = crear();
      const access = crearAccessToken(contexto, SECRETO, '15m');
      await expect(servicio.refrescar(access)).rejects.toThrow(/refresco/);
    });

    it('debe rechazar el token cuando el usuario ya no existe', async () => {
      const { servicio, usuarios } = crear();
      const refresh = crearRefreshToken(contexto, SECRETO, '7d');
      (usuarios.encontrarPorId as jest.Mock).mockResolvedValue(null);
      await expect(servicio.refrescar(refresh)).rejects.toThrow(/ya no existe/);
    });

    it('debe emitir una sesion nueva cuando el refresh es valido', async () => {
      const { servicio } = crear();
      const refresh = crearRefreshToken(contexto, SECRETO, '7d');
      const sesion = await servicio.refrescar(refresh);
      expect(sesion.usuario.id).toBe('u1');
      expect(sesion.access_token).not.toBe(refresh);
    });
  });
});
