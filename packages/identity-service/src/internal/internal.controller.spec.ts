import 'reflect-metadata';
import { InternalController } from './internal.controller';
import type { UsuariosService } from '../usuarios/usuarios.service';
import { ClaveInternaGuard, NotFoundError, ROLES } from '@core/shared';

const usuario = {
  id: 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11',
  nombre: 'Ana',
  correo: 'ana@core.local',
  rol: ROLES.VENDEDOR,
};

function fabricar(): { usuarios: Record<string, jest.Mock>; controller: InternalController } {
  const usuarios = { encontrarPorId: jest.fn(), listarVendedores: jest.fn(), contarVendedores: jest.fn() };
  const controller = new InternalController(usuarios as unknown as UsuariosService);
  return { usuarios, controller };
}

describe('InternalController (internal, R-U-10/11)', () => {
  it('debe devolver el usuario por id cuando existe y lanzar NotFoundError cuando no', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.encontrarPorId.mockResolvedValue(usuario);
    await expect(controller.usuario(usuario.id)).resolves.toEqual(usuario);
    usuarios.encontrarPorId.mockResolvedValue(null);
    await expect(controller.usuario(usuario.id)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe devolver la lista de vendedores', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.listarVendedores.mockResolvedValue([usuario]);
    await expect(controller.vendedores()).resolves.toEqual([usuario]);
    expect(usuarios.listarVendedores).toHaveBeenCalledTimes(1);
  });

  it('debe devolver el conteo de vendedores envuelto', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.contarVendedores.mockResolvedValue(3);
    await expect(controller.contarVendedores()).resolves.toEqual({ vendedores: 3 });
  });

  it('debe proteger todas las rutas con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternalController) as unknown[];
    expect(guards).toBeDefined();
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});