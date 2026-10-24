import { ROLES } from '@core/shared';
import {
  InvitacionesEmpresaController,
  InvitacionesPublicasController,
} from './invitaciones.controller';
import type { InvitacionesService } from './invitaciones.service';

const usuario = { user_id: 'u-emp', email: 'e@core.test', rol: ROLES.EMPRESARIO, tenant_id: 'e1' };

function crearEmpresa() {
  const service = {
    crear: vi.fn(),
    listar: vi.fn(),
    revocar: vi.fn(),
  };
  return {
    controller: new InvitacionesEmpresaController(service as unknown as InvitacionesService),
    service,
  };
}

function crearPublicas() {
  const service = {
    consultar: vi.fn(),
    aceptar: vi.fn(),
  };
  return {
    controller: new InvitacionesPublicasController(service as unknown as InvitacionesService),
    service,
  };
}

describe('InvitacionesEmpresaController', () => {
  it('debe delegar la creación con el empresario autenticado', async () => {
    const { controller, service } = crearEmpresa();
    const dto = { correo: 'v@core.test', tienda_id: 't1', participacion_porcentaje: 25 };
    service.crear.mockResolvedValue({ id: 'i1' });

    await expect(controller.crear('e1', dto, usuario as never)).resolves.toEqual({ id: 'i1' });
    expect(service.crear).toHaveBeenCalledWith('u-emp', 'e1', dto);
  });

  it('debe delegar el listado y la revocación', async () => {
    const { controller, service } = crearEmpresa();
    service.listar.mockResolvedValue({ vendedores: [], invitaciones: [] });
    service.revocar.mockResolvedValue({ ok: true });

    await controller.listar('e1', usuario as never);
    await controller.revocar('e1', 'i1', usuario as never);

    expect(service.listar).toHaveBeenCalledWith('u-emp', 'e1');
    expect(service.revocar).toHaveBeenCalledWith('u-emp', 'e1', 'i1');
  });

  it('debe exigir rol empresario en los tres endpoints', () => {
    for (const metodo of ['crear', 'listar', 'revocar'] as const) {
      expect(
        Reflect.getMetadata(
          'roles_requeridos',
          InvitacionesEmpresaController.prototype[metodo],
        ),
      ).toEqual([ROLES.EMPRESARIO]);
    }
  });
});

describe('InvitacionesPublicasController', () => {
  it('debe delegar la consulta y la aceptación sin autenticación', async () => {
    const { controller, service } = crearPublicas();
    service.consultar.mockResolvedValue({ valida: true });
    service.aceptar.mockResolvedValue({ access_token: 'a' });

    await expect(controller.consultar('token')).resolves.toEqual({ valida: true });
    await expect(
      controller.aceptar('token', { nombre: 'Ana', contrasena: 'UnaClave123' }),
    ).resolves.toEqual({ access_token: 'a' });

    expect(service.consultar).toHaveBeenCalledWith('token');
    expect(service.aceptar).toHaveBeenCalledWith('token', {
      nombre: 'Ana',
      contrasena: 'UnaClave123',
    });
  });
});
