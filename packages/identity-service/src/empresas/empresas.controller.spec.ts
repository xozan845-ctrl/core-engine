import type { Mock } from 'vitest';
import 'reflect-metadata';
import { ROLES } from '@core/shared';
import { EmpresasController } from './empresas.controller';
import type { EmpresasService } from './empresas.service';

const empresa = {
  id: 'e-1',
  nombre: 'Acme',
  slug: 'acme',
  modalidad_logistica: 'hibrida',
  estado: 'activa',
  creado_por: 'u-1',
  creado_en: '2026-10-23T00:00:00.000Z',
};

function respuesta(): { status: Mock; setHeader: Mock } {
  return {
    status: vi.fn().mockReturnThis(),
    setHeader: vi.fn(),
  };
}

describe('EmpresasController', () => {
  const empresas = {
    crear: vi.fn(),
    resolverPara: vi.fn(),
    obtenerPara: vi.fn(),
    crearVendedor: vi.fn(),
    actualizarModalidad: vi.fn(),
    actualizarSlug: vi.fn(),
  };
  const controller = new EmpresasController(empresas as unknown as EmpresasService);
  const empresario = { user_id: 'u-1', email: 'e@core.test', rol: ROLES.EMPRESARIO };

  beforeEach(() => vi.clearAllMocks());

  it('crea una empresa con la modalidad y el slug elegidos', async () => {
    empresas.crear.mockResolvedValue({ id: 'e-1' });
    await controller.crear({ nombre: 'Acme', modalidad_logistica: 'hibrida', slug: 'acme' }, empresario);
    expect(empresas.crear).toHaveBeenCalledWith(
      'u-1',
      'Acme',
      'hibrida',
      'acme',
      expect.objectContaining({ nombre: 'Acme', modalidad_logistica: 'hibrida', slug: 'acme' }),
      undefined,
    );
  });

  it('crea un vendedor con su participacion', async () => {
    const dto = { nombre: 'Ventas', correo: 'ventas@acme.test', contrasena: 'secreto123', participacion_porcentaje: 25 };
    await controller.vendedor('e-1', dto, empresario);
    expect(empresas.crearVendedor).toHaveBeenCalledWith('u-1', 'e-1', dto);
  });

  it('cambia la modalidad logistica para envios futuros', async () => {
    empresas.actualizarModalidad.mockResolvedValue({ id: 'e-1', modalidad_logistica: 'propia' });
    await controller.actualizarLogistica('e-1', { modalidad_logistica: 'propia' }, empresario);
    expect(empresas.actualizarModalidad).toHaveBeenCalledWith('u-1', 'e-1', 'propia');
  });

  it('resuelve la empresa por slug sin redirigir cuando es el actual', async () => {
    empresas.resolverPara.mockResolvedValue({ empresa });
    const res = respuesta();

    await expect(controller.obtener('acme', empresario, res as never)).resolves.toEqual(empresa);
    expect(empresas.resolverPara).toHaveBeenCalledWith('u-1', 'acme', false);
    expect(res.status).not.toHaveBeenCalled();
    expect(res.setHeader).not.toHaveBeenCalled();
  });

  it('permite consulta global al super admin', async () => {
    const global = { user_id: 'sa-1', email: 'sa@core.test', rol: ROLES.SUPER_ADMIN };
    empresas.resolverPara.mockResolvedValue({ empresa });

    await controller.obtener('e-1', global, respuesta() as never);
    expect(empresas.resolverPara).toHaveBeenCalledWith('sa-1', 'e-1', true);
  });

  it('responde 301 con Location cuando el slug consultado es anterior', async () => {
    empresas.resolverPara.mockResolvedValue({
      empresa,
      redirigirA: '/api/v1/empresas/acme-nueva',
    });
    const res = respuesta();

    await controller.obtener('acme-vieja', empresario, res as never);

    expect(res.status).toHaveBeenCalledWith(301);
    expect(res.setHeader).toHaveBeenCalledWith('Location', '/api/v1/empresas/acme-nueva');
  });

  it('cambia el slug público de la empresa', async () => {
    empresas.actualizarSlug.mockResolvedValue({ ...empresa, slug: 'acme-nueva' });
    await expect(
      controller.actualizarSlug('e-1', { slug: 'acme-nueva' }, empresario),
    ).resolves.toEqual({ ...empresa, slug: 'acme-nueva' });
    expect(empresas.actualizarSlug).toHaveBeenCalledWith('u-1', 'e-1', 'acme-nueva');
  });

  it('exige rol empresario para cambiar el slug y la logística', () => {
    expect(Reflect.getMetadata('roles_requeridos', EmpresasController.prototype.actualizarSlug)).toEqual([
      ROLES.EMPRESARIO,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', EmpresasController.prototype.actualizarLogistica)).toEqual([
      ROLES.EMPRESARIO,
    ]);
  });
});
