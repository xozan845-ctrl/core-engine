import 'reflect-metadata';
import { ROLES } from '@core/shared';
import { EmpresasController } from './empresas.controller';
import type { EmpresasService } from './empresas.service';

describe('EmpresasController', () => {
  const empresas = {
    crear: vi.fn(),
    obtenerPara: vi.fn(),
    crearVendedor: vi.fn(),
    actualizarModalidad: vi.fn(),
  };
  const controller = new EmpresasController(empresas as unknown as EmpresasService);
  const empresario = { user_id: 'u-1', email: 'e@core.test', rol: ROLES.EMPRESARIO };

  beforeEach(() => vi.clearAllMocks());

  it('crea una empresa con la modalidad elegida', async () => {
    empresas.crear.mockResolvedValue({ id: 'e-1' });
    await controller.crear({ nombre: 'Acme', modalidad_logistica: 'hibrida' }, empresario);
    expect(empresas.crear).toHaveBeenCalledWith('u-1', 'Acme', 'hibrida');
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

  it('permite consulta global al super admin', async () => {
    const global = { user_id: 'sa-1', email: 'sa@core.test', rol: ROLES.SUPER_ADMIN };
    await controller.obtener('e-1', global);
    expect(empresas.obtenerPara).toHaveBeenCalledWith('sa-1', 'e-1', true);
  });
});
