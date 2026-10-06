import { AuthService } from './auth.service';
import { UsuariosService, Usuario } from '../usuarios/usuarios.service';
import { SesionesRepository } from './sesiones.repository';
import { DomainError, ROLES, crearRefreshToken, crearAccessToken, verificarToken, TokenPayload } from '@core/shared';

/**
 * Registro, login y refresh (doc 4.3): JWT de corta duracion + refresh.
 * El registro publico solo admite vendedor/comprador (RF-01).
 * R-GW-4 / ADR-18: el refresh solo renueva mientras la sesion siga activa.
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
    const sesiones = {
      crear: jest.fn().mockResolvedValue(undefined),
      activaPorJti: jest.fn().mockResolvedValue({
        id: 's-1',
        usuario_id: 'u1',
        jti: 'jti-1',
        creada_en: '',
        expira_en: '',
        revocada_en: null,
      }),
      revocar: jest.fn().mockResolvedValue(1),
      revocarTodas: jest.fn().mockResolvedValue(2),
    } as unknown as SesionesRepository;
    const servicio = new AuthService(usuarios, sesiones);
    return { servicio, usuarios, sesiones: sesiones as unknown as Record<string, jest.Mock> };
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

    it('debe persistir la sesion cuando el registro es exitoso', async () => {
      const { servicio, sesiones } = crear();
      const sesion = await servicio.registrar({
        nombre: 'Ana',
        correo: 'ana@core.test',
        contrasena: 'secreta123',
        rol: ROLES.VENDEDOR,
      });
      expect(sesiones.crear).toHaveBeenCalledTimes(1);
      const [usuarioId, jti, expira] = sesiones.crear.mock.calls[0];
      expect(usuarioId).toBe('u1');
      expect(typeof jti).toBe('string');
      expect(expira.getTime()).toBeGreaterThan(Date.now());
      expect(verificarToken<TokenPayload>(sesion.refresh_token, SECRETO).jti).toBe(jti);
    });
  });

  describe('login', () => {
    it('debe devolver la sesion cuando las credenciales son validas', async () => {
      const { servicio, usuarios, sesiones } = crear();
      const sesion = await servicio.login('ana@core.test', 'secreta123');
      expect(usuarios.verificarCredenciales).toHaveBeenCalledWith('ana@core.test', 'secreta123');
      expect(sesion.usuario.id).toBe('u1');
      expect(sesiones.crear).toHaveBeenCalledTimes(1);
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

    it('debe rechazar el refresh cuando no trae jti de sesion', async () => {
      const { servicio } = crear();
      const refresh = crearRefreshToken(contexto, SECRETO, '7d');
      await expect(servicio.refrescar(refresh)).rejects.toThrow(/sesion asociada/);
    });

    it('debe rechazar el refresh cuando la sesion fue revocada', async () => {
      const { servicio, sesiones } = crear();
      sesiones.activaPorJti.mockResolvedValue(null);
      const refresh = crearRefreshToken(contexto, SECRETO, '7d', 'jti-1');
      await expect(servicio.refrescar(refresh)).rejects.toThrow(/sesion fue cerrada/i);
    });

    it('debe rechazar el refresh cuando la sesion es de otro usuario', async () => {
      const { servicio, sesiones } = crear();
      sesiones.activaPorJti.mockResolvedValue({
        id: 's-9',
        usuario_id: 'otro',
        jti: 'jti-1',
        creada_en: '',
        expira_en: '',
        revocada_en: null,
      });
      const refresh = crearRefreshToken(contexto, SECRETO, '7d', 'jti-1');
      await expect(servicio.refrescar(refresh)).rejects.toThrow(/sesion fue cerrada/i);
    });

    it('debe rechazar el token cuando el usuario ya no existe', async () => {
      const { servicio, usuarios } = crear();
      (usuarios.encontrarPorId as jest.Mock).mockResolvedValue(null);
      const refresh = crearRefreshToken(contexto, SECRETO, '7d', 'jti-1');
      await expect(servicio.refrescar(refresh)).rejects.toThrow(/ya no existe/);
    });

    it('debe emitir una sesion nueva reusando el jti cuando la sesion esta activa', async () => {
      const { servicio, sesiones } = crear();
      const refresh = crearRefreshToken(contexto, SECRETO, '7d', 'jti-1');
      const sesion = await servicio.refrescar(refresh);
      expect(sesion.usuario.id).toBe('u1');
      expect(sesion.access_token).not.toBe(refresh);
      expect(verificarToken<TokenPayload>(sesion.refresh_token, SECRETO).jti).toBe('jti-1');
      // no se crea una sesion nueva (se reutiliza la existente)
      expect(sesiones.crear).not.toHaveBeenCalled();
    });
  });

  describe('cerrarSesion (R-GW-4)', () => {
    it('debe revocar todas las sesiones cuando no se presenta refresh_token', async () => {
      const { servicio, sesiones } = crear();
      await expect(servicio.cerrarSesion('u1')).resolves.toBe(2);
      expect(sesiones.revocarTodas).toHaveBeenCalledWith('u1');
      expect(sesiones.revocar).not.toHaveBeenCalled();
    });

    it('debe revocar solo la sesion del refresh presentado cuando es del usuario', async () => {
      const { servicio, sesiones } = crear();
      const refresh = crearRefreshToken(contexto, SECRETO, '7d', 'jti-1');
      await expect(servicio.cerrarSesion('u1', refresh)).resolves.toBe(1);
      expect(sesiones.revocar).toHaveBeenCalledWith('jti-1', 'u1');
    });

    it('debe ser idempotente cuando el refresh es invalido', async () => {
      const { servicio, sesiones } = crear();
      await expect(servicio.cerrarSesion('u1', 'token-basura')).resolves.toBe(0);
      expect(sesiones.revocar).not.toHaveBeenCalled();
    });

    it('no debe revocar la sesion de otro usuario', async () => {
      const { servicio, sesiones } = crear();
      const refresh = crearRefreshToken({ ...contexto, id: 'otro' }, SECRETO, '7d', 'jti-1');
      await expect(servicio.cerrarSesion('u1', refresh)).resolves.toBe(0);
      expect(sesiones.revocar).not.toHaveBeenCalled();
    });
  });
});
