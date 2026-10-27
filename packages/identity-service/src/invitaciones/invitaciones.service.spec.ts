import { InvitacionesService } from './invitaciones.service';

function crear() {
  const client = { query: vi.fn() };
  const pg = {
    queryOne: vi.fn(),
    query: vi.fn(),
    transaccion: vi.fn(async (fn: (c: unknown) => Promise<unknown>) => fn(client)),
  };
  const auth = {
    crearSesion: vi.fn().mockResolvedValue({
      access_token: 'access',
      refresh_token: 'refresh',
      expira_en: 900,
      usuario: {
        id: 'u1',
        nombre: 'Ana',
        correo: 'ana@core.test',
        rol: 'vendedor',
      },
    }),
  };
  return { servicio: new InvitacionesService(pg as never, auth as never), pg, client, auth };
}

const empate = { rowCount: 1, rows: [{ ok: 1 }] };
const vacio = { rowCount: 0, rows: [] };

const invitacion = {
  id: 'i1',
  empresa_id: 'e1',
  tienda_id: 't1',
  correo: 'ana@core.test',
  participacion_porcentaje: 25,
  expira_en: new Date(Date.now() + 60_000).toISOString(),
  estado: 'pendiente',
  empresa_nombre: 'Empresa Demo',
  tienda_nombre: 'Tienda Principal',
  tienda_empresa_id: 'e1',
  tienda_vendedor_id: null,
};

describe('InvitacionesService', () => {
  it('genera una invitación con token, expiración y URL del frontend', async () => {
    const { servicio, pg, client } = crear();
    pg.queryOne.mockResolvedValueOnce(empate);
    client.query.mockImplementation(async (sql: string) => {
      if (sql.includes('FROM stores.tiendas')) {
        return { rows: [{ id: 't1', nombre: 'Tienda Principal', empresa_id: 'e1', vendedor_id: null }] };
      }
      if (sql.includes('FROM identity.empresa_miembros')) return vacio;
      if (sql.includes('JOIN stores.tiendas t ON t.vendedor_id')) return vacio;
      if (sql.includes('FROM identity.invitaciones_vendedor')) return vacio;
      return { rows: [{ id: 'i1', estado: 'pendiente', expira_en: '2026-10-26T00:00:00.000Z' }] };
    });

    const creada = await servicio.crear('u-emp', 'e1', {
      correo: 'ANA@CORE.TEST',
      tienda_id: 't1',
      participacion_porcentaje: 25,
    });

    expect(creada.correo).toBe('ana@core.test');
    expect(creada.url_invitacion).toMatch(/^http:\/\/localhost:5173\/i\/[A-Za-z0-9_-]+$/);
    expect(creada.url_invitacion).not.toContain('?token=');
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO identity.invitaciones_vendedor'),
      expect.arrayContaining(['e1', 't1', 'ana@core.test', null, 25]),
    );
  });

  it('rechaza una participación inválida y un correo inválido', async () => {
    const { servicio } = crear();
    await expect(
      servicio.crear('u-emp', 'e1', {
        correo: 'ana@core.test',
        tienda_id: 't1',
        participacion_porcentaje: 101,
      }),
    ).rejects.toMatchObject({ codigo: 'PARTICIPACION_INVALIDA' });
    await expect(
      servicio.crear('u-emp', 'e1', {
        correo: 'no-es-correo',
        tienda_id: 't1',
        participacion_porcentaje: 25,
      }),
    ).rejects.toMatchObject({ codigo: 'CORREO_INVALIDO' });
  });

  it('consulta una invitación vigente y rechaza una expirada', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValueOnce(invitacion);
    await expect(servicio.consultar('token')).resolves.toMatchObject({
      valida: true,
      correo: 'ana@core.test',
      empresa_nombre: 'Empresa Demo',
    });

    pg.queryOne.mockResolvedValueOnce({
      ...invitacion,
      expira_en: new Date(Date.now() - 1_000).toISOString(),
    });
    await expect(servicio.consultar('token')).rejects.toMatchObject({
      codigo: 'INVITACION_NO_VALIDA',
    });
  });

  it('acepta la invitación en una sola transacción y devuelve sesión', async () => {
    const { servicio, client, auth } = crear();
    client.query.mockImplementation(async (sql: string) => {
      if (sql.includes('FROM identity.invitaciones_vendedor')) {
        return { rows: [invitacion], rowCount: 1 };
      }
      if (sql.includes('FROM identity.usuarios WHERE LOWER(correo)')) return vacio;
      if (sql.includes('INSERT INTO identity.usuarios')) {
        return {
          rows: [{
            id: 'u1', nombre: 'Ana', correo: 'ana@core.test', rol: 'vendedor',
            tenant_id: 'e1', participacion_porcentaje: 25,
            creado_en: '2026-10-23T00:00:00.000Z',
          }],
          rowCount: 1,
        };
      }
      if (sql.includes('INSERT INTO identity.empresa_miembros')) return empate;
      if (sql.includes('UPDATE stores.tiendas')) return empate;
      return empate;
    });

    const resultado = await servicio.aceptar('token', {
      nombre: 'Ana',
      contrasena: 'UnaClave123',
    });

    expect(resultado.usuario).toMatchObject({
      id: 'u1',
      tienda_id: 't1',
      tienda_nombre: 'Tienda Principal',
      participacion_porcentaje: 25,
    });
    expect(resultado.access_token).toBe('access');
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining('UPDATE stores.tiendas'),
      ['u1', 25, 't1', 'e1'],
    );
    expect(client.query).toHaveBeenCalledWith(
      expect.stringContaining("SET estado='aceptada'"),
      ['u1', 'i1'],
    );
    expect(auth.crearSesion).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'u1', tenant_id: 'e1' }),
    );
  });

  it('rechaza aceptar si el correo ya tiene cuenta', async () => {
    const { servicio, client } = crear();
    client.query.mockImplementation(async (sql: string) => {
      if (sql.includes('FROM identity.invitaciones_vendedor')) {
        return { rows: [invitacion], rowCount: 1 };
      }
      if (sql.includes('FROM identity.usuarios WHERE LOWER(correo)')) return empate;
      return empate;
    });

    await expect(
      servicio.aceptar('token', { nombre: 'Ana', contrasena: 'UnaClave123' }),
    ).rejects.toMatchObject({ codigo: 'CORREO_YA_REGISTRADO' });
  });

  it('lista vendedores/invitaciones y revoca una invitación', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce(empate)
      .mockResolvedValueOnce(empate)
      .mockResolvedValueOnce({ id: 'i1' });
    pg.query
      .mockResolvedValueOnce([{ id: 'u1', nombre: 'Ana' }])
      .mockResolvedValueOnce([{ id: 'i1', estado: 'pendiente' }]);

    await expect(servicio.listar('u-emp', 'e1')).resolves.toMatchObject({
      vendedores: [{ id: 'u1' }],
      invitaciones: [{ id: 'i1' }],
    });
    await expect(servicio.revocar('u-emp', 'e1', 'i1')).resolves.toEqual({ ok: true });

    pg.queryOne.mockReset();
    pg.queryOne.mockResolvedValueOnce(empate).mockResolvedValueOnce(null);
    await expect(servicio.revocar('u-emp', 'e1', 'i2')).rejects.toMatchObject({
      codigo: 'NO_ENCONTRADO',
    });
  });
});
