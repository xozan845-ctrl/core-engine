import { ForbiddenError, NotFoundError } from '@core/shared';
import { EmpresasService } from './empresas.service';

describe('EmpresasService', () => {
  it('actualiza la modalidad cuando el actor es empresario activo', async () => {
    const empresa = { id: 'e-1', modalidad_logistica: 'propia' };
    const pg = { queryOne: vi.fn().mockResolvedValueOnce({ ok: 1 }).mockResolvedValueOnce(empresa) };
    const servicio = new EmpresasService(pg as never, {} as never);

    await expect(servicio.actualizarModalidad('u-1', 'e-1', 'propia')).resolves.toEqual(empresa);
    expect(pg.queryOne).toHaveBeenNthCalledWith(
      2, expect.stringContaining('UPDATE identity.empresas'), ['propia', 'e-1'],
    );
  });

  it('rechaza a quien no es empresario activo de la empresa', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(null) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.actualizarModalidad('u-x', 'e-1', 'hibrida'))
      .rejects.toBeInstanceOf(ForbiddenError);
  });

  it('no modifica una empresa inactiva o inexistente', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValueOnce({ ok: 1 }).mockResolvedValueOnce(null) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.actualizarModalidad('u-1', 'e-1', 'plataforma'))
      .rejects.toBeInstanceOf(NotFoundError);
  });
});
