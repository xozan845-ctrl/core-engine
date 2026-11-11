import type { Mock } from 'vitest';
import { CapabilityDisabledError, ForbiddenError, NotFoundError, ROLES } from '@core/shared';
import { RegistrosService } from './registros.service';

const EMPRESA = '11111111-1111-4111-8111-111111111111';
const PARTE = '22222222-2222-4222-8222-222222222222';
const REGISTRO = '33333333-3333-4333-8333-333333333333';
const AUTOR = '44444444-4444-4444-8444-444444444444';

const actor = { user_id: AUTOR, rol: ROLES.EMPRESARIO, tenant_id: EMPRESA };

const fila = {
  id: REGISTRO,
  empresa_id: EMPRESA,
  parte_id: PARTE,
  tipo: 'receta',
  titulo: 'Receta',
  datos: { medicamento: 'ibuprofeno' },
  adjuntos: [],
  cita_id: null,
  item_id: null,
  autor_id: AUTOR,
  creado_en: '2026-11-11T00:00:00.000Z',
  actualizado_en: '2026-11-11T00:00:00.000Z',
};

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  const capacidades = { activasDeEmpresas: vi.fn() };
  const servicio = new RegistrosService(pg as never, capacidades as never);
  return { servicio, pg, capacidades };
}

describe('RegistrosService (Fase A)', () => {
  it('crea un registro cuando la capacidad del tipo esta activa', async () => {
    const { servicio, pg, capacidades } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ id: PARTE })
      .mockResolvedValueOnce(fila);
    (capacidades.activasDeEmpresas as Mock).mockResolvedValue([
      { empresa_id: EMPRESA, capacidad_codigo: 'recetas', activo: true },
    ]);

    const registro = await servicio.crear(
      EMPRESA,
      PARTE,
      { tipo: 'receta', titulo: 'Receta', datos: { medicamento: 'ibuprofeno' } },
      actor,
    );

    expect(registro).toEqual(fila);
    expect(capacidades.activasDeEmpresas).toHaveBeenCalledWith([EMPRESA], ['recetas']);
    expect((pg.queryOne as Mock).mock.calls.at(-1)?.[0]).toContain('INSERT INTO identity.partes_registros');
  });

  it('usa historia_clinica para los tipos que no son receta', async () => {
    const { servicio, pg, capacidades } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce({
      ...fila,
      tipo: 'historia_clinica',
    });
    (capacidades.activasDeEmpresas as Mock).mockResolvedValue([
      { empresa_id: EMPRESA, capacidad_codigo: 'historia_clinica', activo: true },
    ]);

    await servicio.crear(EMPRESA, PARTE, { tipo: 'historia_clinica' }, actor);
    expect(capacidades.activasDeEmpresas).toHaveBeenCalledWith([EMPRESA], ['historia_clinica']);
  });

  it('rechaza el alta si la capacidad esta inactiva', async () => {
    const { servicio, pg, capacidades } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ id: PARTE });
    (capacidades.activasDeEmpresas as Mock).mockResolvedValue([
      { empresa_id: EMPRESA, capacidad_codigo: 'historia_clinica', activo: false },
    ]);

    await expect(
      servicio.crear(EMPRESA, PARTE, { tipo: 'nota' }, actor),
    ).rejects.toBeInstanceOf(CapabilityDisabledError);
  });

  it('impide operar registros de otra empresa', async () => {
    const { servicio, pg } = crear();
    await expect(
      servicio.listar(EMPRESA, PARTE, { ...actor, tenant_id: '99999999-9999-4999-8999-999999999999' }),
    ).rejects.toBeInstanceOf(ForbiddenError);
    expect(pg.queryOne).not.toHaveBeenCalled();
  });

  it('rechaza una parte inexistente o de otra empresa', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce(null);
    await expect(servicio.obtener(EMPRESA, PARTE, REGISTRO, actor)).rejects.toBeInstanceOf(
      NotFoundError,
    );
  });

  it('lista con filtro de tipo y limite acotado', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock).mockResolvedValueOnce({ id: PARTE });
    (pg.query as Mock).mockResolvedValue([fila]);

    await expect(servicio.listar(EMPRESA, PARTE, actor, 'receta', 500)).resolves.toEqual([fila]);
    const [sql, params] = (pg.query as Mock).mock.calls[0];
    expect(sql).toContain('tipo = $3');
    expect(params).toEqual([EMPRESA, PARTE, 'receta', 100]);
  });

  it('actualiza solo los campos enviados y conserva el resto', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ id: PARTE })
      .mockResolvedValueOnce(fila)
      .mockResolvedValueOnce({ ...fila, titulo: 'Nuevo' });

    const actualizado = await servicio.actualizar(EMPRESA, PARTE, REGISTRO, { titulo: 'Nuevo' }, actor);
    expect(actualizado.titulo).toBe('Nuevo');
    const params = (pg.queryOne as Mock).mock.calls.at(-1)?.[1] as unknown[];
    expect(params[2]).toEqual(JSON.stringify(fila.datos));
  });

  it('archiva el registro y falla si ya no existe', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ id: PARTE })
      .mockResolvedValueOnce({ id: REGISTRO });
    await expect(servicio.eliminar(EMPRESA, PARTE, REGISTRO, actor)).resolves.toEqual({ ok: true });

    (pg.queryOne as Mock).mockResolvedValueOnce({ id: PARTE }).mockResolvedValueOnce(null);
    await expect(servicio.eliminar(EMPRESA, PARTE, REGISTRO, actor)).rejects.toBeInstanceOf(
      NotFoundError,
    );
  });

  it('valida tipo, datos y adjuntos', async () => {
    const { servicio, pg, capacidades } = crear();
    (pg.queryOne as Mock).mockResolvedValue({ id: PARTE });
    (capacidades.activasDeEmpresas as Mock).mockResolvedValue([
      { empresa_id: EMPRESA, capacidad_codigo: 'historia_clinica', activo: true },
    ]);

    await expect(servicio.crear(EMPRESA, PARTE, { tipo: 'Tipo Invalido' }, actor)).rejects.toMatchObject({
      codigo: 'REGISTRO_TIPO_INVALIDO',
    });
    await expect(
      servicio.crear(EMPRESA, PARTE, { tipo: 'nota', adjuntos: ['no-uuid'] }, actor),
    ).rejects.toMatchObject({ codigo: 'REGISTRO_ADJUNTOS_INVALIDOS' });
    await expect(
      servicio.crear(EMPRESA, PARTE, { tipo: 'nota', datos: [] as never }, actor),
    ).rejects.toMatchObject({ codigo: 'REGISTRO_DATOS_INVALIDOS' });
    await expect(
      servicio.crear(EMPRESA, PARTE, { tipo: 'nota', datos: { texto: 'x'.repeat(70 * 1024) } }, actor),
    ).rejects.toMatchObject({ codigo: 'REGISTRO_DATOS_INVALIDOS' });
  });
});
