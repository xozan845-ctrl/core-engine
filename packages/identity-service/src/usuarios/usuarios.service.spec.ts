import { UsuariosService } from './usuarios.service';
import { DomainError, ConflictError, NotFoundError, UnauthorizedError } from '@core/shared';
import * as bcrypt from 'bcryptjs';

/**
 * Registro y perfiles (RF-01): validacion de correo/contrasena, unicidad,
 * hash bcrypt y minima exposicion (el hash jamas sale del servicio).
 */
describe('UsuariosService (registro y perfiles)', () => {
  const fila = {
    id: 'u1',
    nombre: 'Ana',
    correo: 'ana@core.test',
    rol: 'vendedor' as const,
    creado_en: '2026-09-01T00:00:00.000Z',
    contrasena_hash: '$2a$10$abcdefghijklmnopqrstuvABCDEFGHIJKLMNOPQRSTUV1234567890ab',
  };

  const crear = () => {
    const pg = {
      query: jest.fn().mockResolvedValue([]),
      queryOne: jest.fn().mockResolvedValue(null),
    };
    const servicio = new UsuariosService(pg as never);
    return { servicio, pg };
  };

  const datos = { nombre: 'Ana', correo: 'ana@core.test', contrasena: 'secreta123', rol: 'vendedor' as const };

  describe('crear', () => {
    it('debe rechazar el correo cuando no tiene formato valido (normaliza a minusculas)', async () => {
      const { servicio } = crear();
      await expect(servicio.crear({ ...datos, correo: ' Sin Espacios ' })).rejects.toThrow(/correo/i);
      await expect(servicio.crear({ ...datos, correo: 'ana@core' })).rejects.toThrow(DomainError);
    });

    it('debe rechazar la contrasena cuando tiene menos de 8 caracteres', async () => {
      const { servicio } = crear();
      await expect(servicio.crear({ ...datos, contrasena: 'corta' })).rejects.toThrow(/8 caracteres/);
    });

    it('debe devolver ConflictError cuando el correo ya esta registrado', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(fila);
      await expect(servicio.crear(datos)).rejects.toThrow(ConflictError);
      // la validacion ocurre antes del INSERT
      const sqls = (pg.queryOne as jest.Mock).mock.calls.map((c) => c[0] as string);
      expect(sqls.some((s) => s.includes('INSERT INTO identity.usuarios'))).toBe(false);
    });

    it('debe guardar el hash bcrypt y devolver el usuario sin hash cuando el registro es valido', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock)
        .mockResolvedValueOnce(null) // correo no existe
        .mockResolvedValueOnce(fila); // INSERT

      const usuario = await servicio.crear(datos);

      expect(usuario).not.toHaveProperty('contrasena_hash'); // minima exposicion
      const [, params] = (pg.queryOne as jest.Mock).mock.calls[1];
      expect(params[1]).toBe('ana@core.test'); // correo saneado
      expect(await bcrypt.compare('secreta123', params[2] as string)).toBe(true);
    });
  });

  describe('verificarCredenciales', () => {
    it('debe lanzar UnauthorizedError cuando el correo no existe (mismo mensaje que contraseña mala)', async () => {
      const { servicio, pg } = crear();
      (pg.queryOne as jest.Mock).mockResolvedValue(null);
      await expect(servicio.verificarCredenciales('nadia@core.test', 'x')).rejects.toThrow(UnauthorizedError);
    });

    it('debe lanzar UnauthorizedError cuando la contrasena no coincide', async () => {
      const hash = await bcrypt.hash('correcta123', 4);
      const servicio = new UsuariosService({
        queryOne: jest.fn().mockResolvedValue({ ...fila, contrasena_hash: hash }),
      } as never);
      await expect(servicio.verificarCredenciales('ana@core.test', 'otra')).rejects.toThrow(
        UnauthorizedError,
      );
    });

    it('debe devolver el usuario saneado (sin hash) cuando la contrasena coincide', async () => {
      const hash = await bcrypt.hash('correcta123', 4);
      const servicio = new UsuariosService({
        queryOne: jest.fn().mockResolvedValue({ ...fila, contrasena_hash: hash }),
      } as never);
      const usuario = await servicio.verificarCredenciales('ana@core.test', 'correcta123');
      expect(usuario.id).toBe('u1');
      expect(usuario).not.toHaveProperty('contrasena_hash');
    });
  });

  describe('cambiarContrasena', () => {
    const servicioCon = (pg: unknown) => new UsuariosService(pg as never);

    it('debe lanzar NotFoundError cuando el usuario no existe', async () => {
      const pg = { queryOne: jest.fn().mockResolvedValue(null), query: jest.fn() };
      await expect(servicioCon(pg).cambiarContrasena('u404', 'actual123', 'nueva1234')).rejects.toThrow(
        NotFoundError,
      );
    });

    it('debe rechazar cuando la contrasena actual no coincide', async () => {
      const hash = await bcrypt.hash('correcta123', 4);
      const pg = {
        queryOne: jest
          .fn()
          .mockResolvedValueOnce({ ...fila }) // encontrarPorId
          .mockResolvedValueOnce({ ...fila, contrasena_hash: hash }), // encontrarPorCorreo
        query: jest.fn(),
      };
      await expect(servicioCon(pg).cambiarContrasena('u1', 'mala', 'nueva1234')).rejects.toThrow(
        UnauthorizedError,
      );
      expect(pg.query).not.toHaveBeenCalled();
    });

    it('debe rechazar la contrasena nueva corta antes de tocar la BD', async () => {
      const hash = await bcrypt.hash('correcta123', 4);
      const pg = {
        queryOne: jest
          .fn()
          .mockResolvedValueOnce({ ...fila })
          .mockResolvedValueOnce({ ...fila, contrasena_hash: hash }),
        query: jest.fn(),
      };
      await expect(servicioCon(pg).cambiarContrasena('u1', 'correcta123', 'corta')).rejects.toThrow(
        /8 caracteres/,
      );
      expect(pg.query).not.toHaveBeenCalled();
    });

    it('debe actualizar el hash cuando la contrasena actual es valida y la nueva suficiente', async () => {
      const hash = await bcrypt.hash('correcta123', 4);
      const pg = {
        queryOne: jest
          .fn()
          .mockResolvedValueOnce({ ...fila })
          .mockResolvedValueOnce({ ...fila, contrasena_hash: hash }),
        query: jest.fn().mockResolvedValue([]),
      };
      await servicioCon(pg).cambiarContrasena('u1', 'correcta123', 'nueva1234');

      expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('UPDATE identity.usuarios'), [
        expect.any(String),
        'u1',
      ]);
      const nuevoHash = (pg.query as jest.Mock).mock.calls[0][1][0] as string;
      expect(await bcrypt.compare('nueva1234', nuevoHash)).toBe(true);
    });
  });

  describe('actualizarPerfil', () => {
    it('debe normalizar el correo y rechazarlo si no es valido', async () => {
      const pg = { queryOne: jest.fn(), query: jest.fn().mockResolvedValue([]) };
      const servicio = new UsuariosService(pg as never);

      await expect(servicio.actualizarPerfil('u1', { correo: 'MALO' })).rejects.toThrow(/correo/i);
      await servicio.actualizarPerfil('u1', { correo: ' Nueva@Core.TEST ' });
      expect((pg.query as jest.Mock).mock.calls[0][1]).toEqual(['nueva@core.test', 'u1']);
    });

    it('debe no hacer nada cuando los datos estan vacios', async () => {
      const pg = { queryOne: jest.fn(), query: jest.fn() };
      const servicio = new UsuariosService(pg as never);
      await servicio.actualizarPerfil('u1', {});
      expect(pg.query).not.toHaveBeenCalled();
    });
  });

  describe('consultas de rol', () => {
    it('debe contar cero vendedores cuando la vista no devuelve nada', async () => {
      const pg = { queryOne: jest.fn().mockResolvedValue(null), query: jest.fn() };
      expect(await new UsuariosService(pg as never).contarVendedores()).toBe(0);
    });

    it('debe responder si existe admin', async () => {
      const pg = { queryOne: jest.fn().mockResolvedValue({ '?column?': 1 }), query: jest.fn() };
      expect(await new UsuariosService(pg as never).existeAdmin()).toBe(true);
    });

    it('debe listar con filtro de rol y tenant cuando se piden', async () => {
      const pg = { queryOne: jest.fn(), query: jest.fn().mockResolvedValue([]) };
      const servicio = new UsuariosService(pg as never);
      await servicio.listar({ rol: 'admin', tenant_id: 't1' });
      const [sql, params] = (pg.query as jest.Mock).mock.calls[0];
      expect(sql).toContain('WHERE rol = $1 AND tenant_id = $2');
      expect(params).toEqual(['admin', 't1']);
    });
  });
});
