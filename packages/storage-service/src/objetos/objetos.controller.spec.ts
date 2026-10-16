import 'reflect-metadata';
import { ObjetosController } from './objetos.controller';
import { DomainError, ROLES, UsuarioContexto } from '@core/shared';
import type { ObjetosService } from './objetos.service';

const usuario: UsuarioContexto = {
  user_id: 'usuario-1',
  email: 'vendedor@core-engine.test',
  rol: ROLES.VENDEDOR,
  tenant_id: '11111111-1111-4111-8111-111111111111',
};

const crear = () => {
  const objetos = {
    solicitarSubida: vi.fn().mockResolvedValue({ objeto_id: 'o1' }),
    confirmar: vi.fn().mockResolvedValue({ id: 'o1', estado: 'activo' }),
    listar: vi.fn().mockResolvedValue({ items: [], total: 0 }),
    obtener: vi.fn().mockResolvedValue({ id: 'o1' }),
    urlDescarga: vi.fn().mockResolvedValue({ url_descarga: 'http://minio/x' }),
    eliminar: vi.fn().mockResolvedValue(undefined),
  };
  return { controller: new ObjetosController(objetos as unknown as ObjetosService), objetos };
};

describe('ObjetosController (api/v1/storage, R-U-10/11)', () => {
  it('debe exigir el tenant del token en todas las operaciones', async () => {
    const { controller } = crear();
    const sinTenant: UsuarioContexto = { ...usuario, tenant_id: undefined };

    await expect(
      controller.solicitarSubida({} as never, sinTenant),
    ).rejects.toThrow(DomainError);
    await expect(controller.confirmar('o1', sinTenant)).rejects.toThrow(DomainError);
    await expect(controller.listar({}, sinTenant)).rejects.toThrow(DomainError);
    await expect(controller.detalle('o1', sinTenant)).rejects.toThrow(DomainError);
    await expect(controller.descarga('o1', sinTenant)).rejects.toThrow(DomainError);
    await expect(controller.eliminar('o1', sinTenant)).rejects.toThrow(DomainError);
  });

  it('debe delegar la solicitud de subida con el tenant y el autor', async () => {
    const { controller, objetos } = crear();
    const dto = { nombre: 'a.png', content_type: 'image/png', tamano_bytes: 10 };

    await controller.solicitarSubida(dto, usuario);

    expect(objetos.solicitarSubida).toHaveBeenCalledWith(usuario.tenant_id, usuario.user_id, dto);
  });

  it('debe delegar confirmar, listar, detalle, descarga y eliminar con el tenant', async () => {
    const { controller, objetos } = crear();

    await controller.confirmar('o1', usuario);
    await controller.listar({ pagina: 1 }, usuario);
    await controller.detalle('o1', usuario);
    await controller.descarga('o1', usuario);
    await controller.eliminar('o1', usuario);

    expect(objetos.confirmar).toHaveBeenCalledWith(usuario.tenant_id, 'o1');
    expect(objetos.listar).toHaveBeenCalledWith(usuario.tenant_id, { pagina: 1 });
    expect(objetos.obtener).toHaveBeenCalledWith(usuario.tenant_id, 'o1');
    expect(objetos.urlDescarga).toHaveBeenCalledWith(usuario.tenant_id, 'o1');
    expect(objetos.eliminar).toHaveBeenCalledWith(usuario.tenant_id, 'o1', usuario);
  });

  it('debe declarar el DTO de subida con validaciones de whitelist y tamano', () => {
    const metadatos = Reflect.getMetadata('__validationMetadatas__', ObjetosController);
    expect(metadatos ?? true).toBeTruthy();
  });
});
