import { ConflictError, ForbiddenError, ROLES } from '@core/shared';
import { CapacidadesService } from './capacidades.service';

const actor = { user_id: 'u-1', rol: ROLES.EMPRESARIO, tenant_id: 'e-1' };
const capacidad = {
  capacidad_codigo: 'agenda_recursos',
  nombre: 'Agenda y recursos',
  modulo: 'scheduling',
  activo: true,
  config: {},
  actualizado_en: '2026-11-06T00:00:00.000Z',
};

describe('CapacidadesService (ADR-0001)', () => {
  it('lista las capacidades del empresario activo', async () => {
    const pg = {
      queryOne: vi.fn().mockResolvedValue({ ok: 1 }),
      query: vi.fn().mockResolvedValue([capacidad]),
    };
    const servicio = new CapacidadesService(pg as never);

    await expect(servicio.listar('e-1', actor)).resolves.toEqual([capacidad]);
    expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('empresa_miembros'), ['e-1', 'u-1']);
  });

  it('rechaza listar una empresa que no pertenece al actor', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(null), query: vi.fn() };
    const servicio = new CapacidadesService(pg as never);

    await expect(servicio.listar('e-2', actor)).rejects.toBeInstanceOf(ForbiddenError);
    expect(pg.query).not.toHaveBeenCalled();
  });

  it('permite al administrador actualizar una capacidad existente', async () => {
    const actualizado = { ...capacidad, activo: false, config: { motivo: 'piloto' } };
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ codigo: 'agenda_recursos' })
        .mockResolvedValueOnce(actualizado),
    };
    const servicio = new CapacidadesService(pg as never);

    await expect(
      servicio.actualizar('e-1', 'agenda_recursos', false, { motivo: 'piloto' }, {
        user_id: 'admin',
        rol: ROLES.ADMIN,
      }),
    ).resolves.toEqual(actualizado);
    expect(pg.queryOne).toHaveBeenLastCalledWith(expect.stringContaining('ON CONFLICT'), [
      'e-1',
      'agenda_recursos',
      false,
      JSON.stringify({ motivo: 'piloto' }),
    ]);
  });

  it('permite al empresario actualizar su capacidad y usa config vacia por defecto', async () => {
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce({ codigo: 'agenda_recursos' })
        .mockResolvedValueOnce(capacidad),
    };
    const servicio = new CapacidadesService(pg as never);

    await expect(servicio.actualizar('e-1', 'agenda_recursos', true, undefined, actor)).resolves.toEqual(capacidad);
    expect(pg.queryOne).toHaveBeenLastCalledWith(expect.stringContaining('ON CONFLICT'), [
      'e-1',
      'agenda_recursos',
      true,
      null,
    ]);
  });

  it('rechaza una capacidad inexistente o inactiva', async () => {
    const pg = {
      queryOne: vi.fn()
        .mockResolvedValueOnce({ ok: 1 })
        .mockResolvedValueOnce(null),
    };
    const servicio = new CapacidadesService(pg as never);

    await expect(servicio.actualizar('e-1', 'desconocida', true, undefined, actor)).rejects.toBeInstanceOf(ConflictError);
  });
});
