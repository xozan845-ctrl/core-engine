import 'reflect-metadata';
import { UsuariosController } from './usuarios.controller';
import type { UsuariosService } from './usuarios.service';
import { NotFoundError, ROLES } from '@core/shared';

const usuario = {
  id: 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11',
  nombre: 'Ana',
  correo: 'ana@core.local',
  rol: ROLES.VENDEDOR,
};

function fabricar(): { usuarios: Record<string, jest.Mock>; controller: UsuariosController } {
  const usuarios = { listar: jest.fn(), encontrarPorId: jest.fn() };
  const controller = new UsuariosController(usuarios as unknown as UsuariosService);
  return { usuarios, controller };
}

describe('UsuariosController (api/v1/usuarios, R-U-10/11)', () => {
  it('debe listar los usuarios que entrega el servicio', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.listar.mockResolvedValue([usuario]);
    await expect(controller.listar()).resolves.toEqual([usuario]);
    expect(usuarios.listar).toHaveBeenCalledTimes(1);
  });

  it('debe devolver el perfil cuando el contexto es admin', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.encontrarPorId.mockResolvedValue(usuario);
    await expect(controller.obtener(usuario.id, { user_id: 'u-admin', email: 'a@c', rol: ROLES.ADMIN } as never)).resolves.toEqual(usuario);
    expect(usuarios.encontrarPorId).toHaveBeenCalledWith(usuario.id);
  });

  it('debe devolver el perfil cuando es el propio usuario', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.encontrarPorId.mockResolvedValue(usuario);
    await expect(controller.obtener(usuario.id, { user_id: usuario.id, email: 'ana@core.local', rol: ROLES.VENDEDOR } as never)).resolves.toEqual(usuario);
  });

  it('debe ocultar con NotFoundError el perfil ajeno o inexistente', async () => {
    const { usuarios, controller } = fabricar();
    usuarios.encontrarPorId.mockResolvedValue(null);
    await expect(
      controller.obtener(usuario.id, { user_id: 'otro', email: 'o@c', rol: ROLES.VENDEDOR } as never),
    ).rejects.toBeInstanceOf(NotFoundError);
    await expect(
      controller.obtener(usuario.id, { user_id: 'u-admin', email: 'a@c', rol: ROLES.ADMIN } as never),
    ).rejects.toBeInstanceOf(NotFoundError);
  });
});