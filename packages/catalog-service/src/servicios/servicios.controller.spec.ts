import { DomainError, NotFoundError, ROLES } from '@core/shared';
import { ServiciosController } from './servicios.controller';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
const servicio = { id: 's1' };

function crear() {
  const svc = {
    listar: vi.fn().mockResolvedValue({ items: [servicio], total: 1, pagina: 1, limite: 20, paginas: 1 }),
    encontrarPorId: vi.fn().mockResolvedValue(servicio),
    crear: vi.fn().mockResolvedValue(servicio),
    actualizar: vi.fn().mockResolvedValue(servicio),
    eliminar: vi.fn().mockResolvedValue(undefined),
  };
  return { controlador: new ServiciosController(svc as never), svc };
}

const empresario = {
  user_id: 'u1',
  email: 'e@test.dev',
  rol: ROLES.EMPRESARIO,
  tenant_id: EMPRESA,
};
const admin = { user_id: 'a1', email: 'a@test.dev', rol: ROLES.ADMIN };

describe('ServiciosController (R-PS-02)', () => {
  it('lista con el tenant del empresario e ignora otra empresa', async () => {
    const { controlador, svc } = crear();
    await controlador.listar({ q: 'x' } as never, empresario);
    expect(svc.listar).toHaveBeenCalledWith({ q: 'x', empresa_id: EMPRESA });

    await expect(
      controlador.listar({ empresa_id: 'otra' } as never, empresario),
    ).rejects.toMatchObject({ codigo: 'EMPRESA_NO_AUTORIZADA' });
  });

  it('exige empresa al admin cuando no la indica', async () => {
    const { controlador } = crear();
    await expect(controlador.listar({} as never, admin)).rejects.toMatchObject({
      codigo: 'EMPRESA_REQUERIDA',
    });
  });

  it('lista para admin con empresa explícita', async () => {
    const { controlador, svc } = crear();
    await controlador.listar({ empresa_id: EMPRESA } as never, admin);
    expect(svc.listar).toHaveBeenCalledWith({ empresa_id: EMPRESA });
  });

  it('obtiene el detalle o lanza 404', async () => {
    const { controlador, svc } = crear();
    await expect(controlador.detalle('s1', undefined, empresario)).resolves.toBe(servicio);
    expect(svc.encontrarPorId).toHaveBeenCalledWith('s1', EMPRESA);

    svc.encontrarPorId.mockResolvedValueOnce(null);
    await expect(controlador.detalle('s2', undefined, empresario)).rejects.toBeInstanceOf(
      NotFoundError,
    );
  });

  it('crea con el tenant del empresario', async () => {
    const { controlador, svc } = crear();
    await controlador.crear({ codigo: 'SERV-1' } as never, empresario);
    expect(svc.crear).toHaveBeenCalledWith({ codigo: 'SERV-1' }, EMPRESA);
  });

  it('crea para admin solo si indica empresa', async () => {
    const { controlador, svc } = crear();
    await expect(
      controlador.crear({ codigo: 'SERV-1' } as never, admin),
    ).rejects.toMatchObject({ codigo: 'EMPRESA_REQUERIDA' });
    await controlador.crear({ codigo: 'SERV-1', empresa_id: EMPRESA } as never, admin);
    expect(svc.crear).toHaveBeenCalledWith(
      { codigo: 'SERV-1', empresa_id: EMPRESA },
      EMPRESA,
    );
  });

  it('actualiza y elimina en el ámbito correcto', async () => {
    const { controlador, svc } = crear();
    await controlador.actualizar('s1', { nombre: 'Nuevo' } as never, undefined, empresario);
    expect(svc.actualizar).toHaveBeenCalledWith('s1', { nombre: 'Nuevo' }, EMPRESA);

    await controlador.eliminar('s1', undefined, empresario);
    expect(svc.eliminar).toHaveBeenCalledWith('s1', EMPRESA);
  });

  it('exige tenant a un empresario sin empresa', async () => {
    const { controlador } = crear();
    await expect(
      controlador.listar({} as never, { ...empresario, tenant_id: undefined }),
    ).rejects.toBeInstanceOf(DomainError);
    await expect(
      controlador.crear({ codigo: 'SERV-1' } as never, { ...empresario, tenant_id: undefined }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });
});
