import { ForbiddenError, NotFoundError } from '@core/shared';
import { EmpresasService } from './empresas.service';

describe('EmpresasService', () => {
  it('crea empresa, membresia y activa el tenant en una transaccion', async () => {
    const empresa = { id: 'e-1', nombre: 'Acme', modalidad_logistica: 'hibrida' };
    const client = {
      query: vi.fn()
        .mockResolvedValueOnce({ rows: [empresa] })
        .mockResolvedValueOnce({ rows: [] })
        .mockResolvedValueOnce({ rows: [] }),
    };
    const pg = { transaccion: vi.fn(async (fn: (c: typeof client) => unknown) => fn(client)) };
    const servicio = new EmpresasService(pg as never, {} as never);

    await expect(servicio.crear('u-1', '  Acme  ', 'hibrida')).resolves.toEqual(empresa);
    expect(client.query).toHaveBeenNthCalledWith(
      1, expect.stringContaining('INSERT INTO identity.empresas'), ['Acme', 'hibrida', 'u-1'],
    );
    expect(client.query).toHaveBeenCalledTimes(3);
  });

  it('crea un vendedor cuando el empresario pertenece a la empresa', async () => {
    const usuario = { id: 'v-1', rol: 'vendedor' };
    const pg = { queryOne: vi.fn().mockResolvedValue({ ok: 1 }) };
    const usuarios = { crearVendedorEnEmpresa: vi.fn().mockResolvedValue(usuario) };
    const servicio = new EmpresasService(pg as never, usuarios as never);
    const datos = {
      nombre: 'Venta', correo: 'venta@acme.test', contrasena: 'secreto123',
      participacion_porcentaje: 25,
    };

    await expect(servicio.crearVendedor('u-1', 'e-1', datos))
      .resolves.toEqual({ ...usuario, participacion_porcentaje: 25 });
    expect(usuarios.crearVendedorEnEmpresa).toHaveBeenCalledWith({ ...datos, empresa_id: 'e-1' });
  });

  it('impide crear vendedores a quien no es empresario activo', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(null) };
    const usuarios = { crearVendedorEnEmpresa: vi.fn() };
    const servicio = new EmpresasService(pg as never, usuarios as never);
    await expect(servicio.crearVendedor('u-x', 'e-1', {
      nombre: 'Venta', correo: 'venta@acme.test', contrasena: 'secreto123',
      participacion_porcentaje: 25,
    })).rejects.toBeInstanceOf(ForbiddenError);
    expect(usuarios.crearVendedorEnEmpresa).not.toHaveBeenCalled();
  });

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

  it('consulta una empresa visible para un miembro activo', async () => {
    const empresa = { id: 'e-1', modalidad_logistica: 'plataforma' };
    const pg = { queryOne: vi.fn().mockResolvedValueOnce({ ok: 1 }).mockResolvedValueOnce(empresa) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.obtenerPara('u-1', 'e-1', false)).resolves.toEqual(empresa);
    expect(pg.queryOne).toHaveBeenCalledTimes(2);
  });

  it('oculta una empresa a quien no tiene membresia', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValue(null) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.obtenerPara('u-x', 'e-1', false)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('permite consulta global y maneja empresa inexistente', async () => {
    const pg = { queryOne: vi.fn().mockResolvedValueOnce(null) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.obtenerPara('sa-1', 'e-x', true)).rejects.toBeInstanceOf(NotFoundError);
    expect(pg.queryOne).toHaveBeenCalledTimes(1);
  });

  it('expone la consulta interna por id', async () => {
    const empresa = { id: 'e-1', modalidad_logistica: 'propia' };
    const pg = { queryOne: vi.fn().mockResolvedValue(empresa) };
    const servicio = new EmpresasService(pg as never, {} as never);
    await expect(servicio.obtener('e-1')).resolves.toEqual(empresa);
  });
});
