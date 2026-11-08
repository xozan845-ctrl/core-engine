import { PersonalController, PersonalPublicController } from './personal.controller';

function crear() {
  const personal = {
    crearRol: vi.fn(), listarRoles: vi.fn(), actualizarRol: vi.fn(), crearPuesto: vi.fn(), listarPuestos: vi.fn(),
    asignar: vi.fn(), listarAsignaciones: vi.fn(), crearInvitacion: vi.fn(), listarInvitaciones: vi.fn(),
    revocarInvitacion: vi.fn(), consultarInvitacion: vi.fn(), aceptarInvitacion: vi.fn(),
  };
  return { personal, controller: new PersonalController(personal as never), publico: new PersonalPublicController(personal as never) };
}

const actor = { user_id: 'u-1', rol: 'empresario', tenant_id: 'e-1' } as never;

describe('PersonalController', () => {
  it('delega la administración completa de personal al servicio', async () => {
    const { controller, personal } = crear();
    const dto = {} as never;
    await controller.crearRol('e-1', dto, actor);
    await controller.listarRoles('e-1', actor);
    await controller.actualizarRol('e-1', 'r-1', { activo: true } as never, actor);
    await controller.crearPuesto('e-1', dto, actor);
    await controller.listarPuestos('e-1', actor);
    await controller.asignar('e-1', dto, actor);
    await controller.listarAsignaciones('e-1', actor);
    await controller.crearInvitacion('e-1', dto, actor);
    await controller.listarInvitaciones('e-1', actor);
    await controller.revocarInvitacion('e-1', 'i-1', actor);
    expect(personal.crearRol).toHaveBeenCalledWith(actor, 'e-1', dto);
    expect(personal.actualizarRol).toHaveBeenCalledWith(actor, 'e-1', 'r-1', true);
    expect(personal.revocarInvitacion).toHaveBeenCalledWith(actor, 'e-1', 'i-1');
  });

  it('declara las rutas administrativas con rol empresario', () => {
    expect(Reflect.getMetadata('roles_requeridos', PersonalController.prototype.crearRol)).toEqual(['super_admin', 'admin', 'empresario']);
    expect(Reflect.getMetadata('roles_requeridos', PersonalController.prototype.crearInvitacion)).toEqual(['super_admin', 'admin', 'empresario']);
  });
});

describe('PersonalPublicController', () => {
  it('delega consulta y aceptación de enlaces', async () => {
    const { publico, personal } = crear();
    await publico.consultar('token');
    await publico.aceptar('token', {} as never);
    expect(personal.consultarInvitacion).toHaveBeenCalledWith('token');
    expect(personal.aceptarInvitacion).toHaveBeenCalledWith('token', {});
  });
});
