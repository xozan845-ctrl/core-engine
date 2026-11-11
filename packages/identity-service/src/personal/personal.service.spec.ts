import { obtenerTienda, tiendasPorIds } from '@core/shared';
import { PersonalService } from './personal.service';

vi.mock('@core/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@core/shared')>();
  return { ...actual, obtenerTienda: vi.fn(), tiendasPorIds: vi.fn() };
});

const empresario = { user_id: 'u-owner', rol: 'empresario' as const, tenant_id: 'e-1' };

function crear() {
  const client = { query: vi.fn() };
  const pg = {
    queryOne: vi.fn(),
    query: vi.fn(),
    transaccion: vi.fn(async (fn: (client: unknown) => Promise<unknown>) => fn(client)),
  };
  const auth = { crearSesion: vi.fn().mockResolvedValue({ access_token: 'a', refresh_token: 'r', expira_en: 900, usuario: { id: 'u-new', rol: 'operativo' } }) };
  const email = { send: vi.fn().mockResolvedValue(undefined) };
  return { servicio: new PersonalService(pg as never, auth as never, email as never), pg, auth, email, client };
}

describe('PersonalService', () => {
  beforeEach(() => {
    vi.mocked(obtenerTienda).mockReset();
    vi.mocked(tiendasPorIds).mockReset();
    vi.mocked(obtenerTienda).mockResolvedValue({
      estado: 'ok',
      tienda: { id: 't-1', nombre: 'Tienda', empresa_id: 'e-1', vendedor_id: null },
    });
    vi.mocked(tiendasPorIds).mockResolvedValue([]);
  });

  it('crea roles con permisos JSON normalizados y deduplicados', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValue({ id: 'r-1', empresa_id: 'e-1', codigo: 'estilista', nombre: 'Estilista', permisos: { version: 1, acciones: ['agenda.leer'] }, activo: true });

    const rol = await servicio.crearRol(empresario, 'e-1', {
      codigo: ' Estilista ',
      nombre: 'Estilista',
      permisos: { acciones: ['agenda.leer', 'agenda.leer'] },
    });

    expect(rol.id).toBe('r-1');
    expect(pg.queryOne).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO identity.roles_empresa'),
      expect.arrayContaining(['e-1', 'estilista', 'Estilista', expect.any(String), 'u-owner']),
    );
    expect(JSON.parse(pg.queryOne.mock.calls[1][1][4])).toEqual({ version: 1, acciones: ['agenda.leer'] });
  });

  it('rechaza permisos que no tienen acciones válidas', async () => {
    const { servicio, pg } = crear();
    // La validación de permisos ocurre después de verificar al empresario.
    // En este caso no necesitamos simular una respuesta de persistencia.
    pg.queryOne.mockResolvedValue({});
    await expect(servicio.crearRol(empresario, 'e-1', {
      codigo: 'cajero', nombre: 'Cajero', permisos: { acciones: ['*'] },
    })).rejects.toMatchObject({ codigo: 'PERMISOS_INVALIDOS' });
  });

  it('crea un puesto solo si la tienda pertenece al mismo tenant', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({})
      .mockResolvedValueOnce({ id: 'p-1', empresa_id: 'e-1', codigo: 'caja_1', nombre: 'Caja 1', tipo: 'caja', tienda_id: 't-1', metadata: {}, activo: true });

    await expect(servicio.crearPuesto(empresario, 'e-1', { codigo: 'caja_1', nombre: 'Caja 1', tienda_id: 't-1' })).resolves.toMatchObject({ id: 'p-1' });
    expect(obtenerTienda).toHaveBeenCalledWith('t-1');
  });

  it('exige un alcance al crear una asignación de personal', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({})
      .mockResolvedValueOnce({ id: 'u-1' })
      .mockResolvedValueOnce({ id: 'r-1' });

    await expect(servicio.asignar(empresario, 'e-1', { usuario_id: 'u-1', rol_empresa_id: 'r-1' })).rejects.toMatchObject({ codigo: 'ALCANCE_REQUERIDO' });
  });

  it('genera un enlace de invitación de incorporación o contratación', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({})
      .mockResolvedValueOnce({ id: 'r-1' })
      .mockResolvedValueOnce({ id: 'i-1', empresa_id: 'e-1', correo: 'ana@core.test', nombre: 'Ana', tipo: 'contratacion', rol_empresa_id: 'r-1', rol_nombre: 'Estilista', tienda_id: 't-1', puesto_id: null, estado: 'pendiente', expira_en: '2026-11-11T00:00:00Z', empresa_nombre: 'Salón' });

    const invitacion = await servicio.crearInvitacion(empresario, 'e-1', {
      correo: 'ANA@CORE.TEST', tipo: 'contratacion', rol_empresa_id: 'r-1', tienda_id: 't-1',
    });

    expect(invitacion.url_invitacion).toMatch(/^http:\/\/localhost:5173\/incorporacion\//);
    expect(invitacion.tipo).toBe('contratacion');
    expect(pg.queryOne).toHaveBeenCalledWith(expect.stringContaining('INSERT INTO identity.invitaciones_personal'), expect.arrayContaining(['e-1', 'ana@core.test', 'contratacion', 'r-1', 't-1']));
  });

  it('envía el enlace de incorporación por correo', async () => {
    const { servicio, pg, email } = crear();
    pg.queryOne
      .mockResolvedValueOnce({})
      .mockResolvedValueOnce({ id: 'r-1' })
      .mockResolvedValueOnce({ id: 'i-1', empresa_id: 'e-1', correo: 'ana@core.test', nombre: 'Ana', tipo: 'contratacion', rol_empresa_id: 'r-1', rol_nombre: 'Estilista', tienda_id: 't-1', puesto_id: null, estado: 'pendiente', expira_en: '2026-11-11T00:00:00Z', empresa_nombre: 'Salón' });

    const invitacion = await servicio.crearInvitacion(empresario, 'e-1', {
      correo: 'ana@core.test', tipo: 'contratacion', rol_empresa_id: 'r-1', tienda_id: 't-1',
    });

    expect(email.send).toHaveBeenCalledWith({
      to: 'ana@core.test',
      subject: 'Invitación a Salón',
      text: expect.stringContaining(invitacion.url_invitacion),
      html: expect.stringContaining(invitacion.url_invitacion),
    });
  });

  it('mantiene la invitación si el correo falla', async () => {
    const { servicio, pg, email } = crear();
    pg.queryOne
      .mockResolvedValueOnce({})
      .mockResolvedValueOnce({ id: 'r-1' })
      .mockResolvedValueOnce({ id: 'i-1', empresa_id: 'e-1', correo: 'ana@core.test', nombre: 'Ana', tipo: 'contratacion', rol_empresa_id: 'r-1', rol_nombre: 'Estilista', tienda_id: 't-1', puesto_id: null, estado: 'pendiente', expira_en: '2026-11-11T00:00:00Z', empresa_nombre: 'Salón' });
    email.send.mockRejectedValue(new Error('SMTP caído'));
    const spy = vi.spyOn(console, 'error').mockImplementation(() => undefined);

    await expect(
      servicio.crearInvitacion(empresario, 'e-1', {
        correo: 'ana@core.test', tipo: 'contratacion', rol_empresa_id: 'r-1', tienda_id: 't-1',
      }),
    ).resolves.toMatchObject({ url_invitacion: expect.stringContaining('/incorporacion/') });
    spy.mockRestore();
  });

  it('lista y activa/desactiva roles y puestos del tenant', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValue({ id: 'r-1', activo: false });
    pg.query.mockResolvedValueOnce([{ id: 'r-1' }]).mockResolvedValueOnce([{ id: 'p-1' }]);

    await expect(servicio.listarRoles(empresario, 'e-1')).resolves.toEqual([{ id: 'r-1' }]);
    await expect(servicio.actualizarRol(empresario, 'e-1', 'r-1', false)).resolves.toMatchObject({ id: 'r-1' });
    await expect(servicio.listarPuestos(empresario, 'e-1')).resolves.toEqual([{ id: 'p-1' }]);
  });

  it('rechaza administrar otra empresa o usar referencias de otro tenant', async () => {
    const { servicio, pg } = crear();
    await expect(servicio.listarRoles(empresario, 'e-2')).rejects.toMatchObject({ codigo: 'ACCESO_DENEGADO' });
    pg.queryOne.mockResolvedValueOnce({});
    vi.mocked(obtenerTienda).mockResolvedValue({ estado: 'no_encontrada' });
    await expect(servicio.crearPuesto(empresario, 'e-1', { codigo: 'caja_2', nombre: 'Caja 2', tienda_id: 't-ajena' })).rejects.toMatchObject({ codigo: 'NO_ENCONTRADO' });
  });

  it('crea asignaciones con tienda y puesto y actualiza la membresía', async () => {
    const { servicio, pg } = crear();
    pg.queryOne
      .mockResolvedValueOnce({}) // empresario
      .mockResolvedValueOnce({}) // miembro
      .mockResolvedValueOnce({ id: 'r-1' })
      .mockResolvedValueOnce({ id: 'p-1', tienda_id: 't-1' })
      .mockResolvedValueOnce({ id: 'a-1', usuario_id: 'u-1' });
    await expect(servicio.asignar(empresario, 'e-1', { usuario_id: 'u-1', rol_empresa_id: 'r-1', tienda_id: 't-1', puesto_id: 'p-1' })).resolves.toMatchObject({ id: 'a-1' });
  });

  it('lista asignaciones y revoca invitaciones', async () => {
    const { servicio, pg } = crear();
    pg.query.mockResolvedValueOnce([{ id: 'a-1' }]).mockResolvedValueOnce([{ id: 'i-1' }]);
    pg.queryOne.mockResolvedValueOnce({}).mockResolvedValueOnce({}).mockResolvedValueOnce({}).mockResolvedValueOnce({ id: 'i-1' });
    await expect(servicio.listarAsignaciones(empresario, 'e-1')).resolves.toEqual([{ id: 'a-1' }]);
    await expect(servicio.listarInvitaciones(empresario, 'e-1')).resolves.toEqual([{ id: 'i-1' }]);
    await expect(servicio.revocarInvitacion(empresario, 'e-1', 'i-1')).resolves.toEqual({ ok: true });
  });

  it('resuelve el nombre de la tienda por API interna al listar asignaciones', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValue({});
    pg.query.mockResolvedValue([{ id: 'a-1', tienda_id: 't-1' }]);
    vi.mocked(tiendasPorIds).mockResolvedValue([
      { id: 't-1', nombre: 'Tienda Centro', empresa_id: 'e-1', vendedor_id: null },
    ]);

    await expect(servicio.listarAsignaciones(empresario, 'e-1')).resolves.toEqual([
      { id: 'a-1', tienda_id: 't-1', tienda_nombre: 'Tienda Centro' },
    ]);
  });

  it('degrada el nombre de la tienda si stores no responde', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValue({});
    pg.query.mockResolvedValue([{ id: 'a-1', tienda_id: 't-1' }]);
    vi.mocked(tiendasPorIds).mockResolvedValue(null);

    await expect(servicio.listarAsignaciones(empresario, 'e-1')).resolves.toEqual([
      { id: 'a-1', tienda_id: 't-1', tienda_nombre: null },
    ]);
  });

  it('falla cerrado al validar una tienda si stores no responde', async () => {
    const { servicio, pg } = crear();
    pg.queryOne.mockResolvedValueOnce({});
    vi.mocked(obtenerTienda).mockResolvedValue({ estado: 'error' });

    await expect(
      servicio.crearPuesto(empresario, 'e-1', { codigo: 'caja_3', nombre: 'Caja 3', tienda_id: 't-1' }),
    ).rejects.toMatchObject({ codigo: 'SERVICIO_NO_DISPONIBLE' });
  });

  it('consulta y acepta una incorporación creando cuenta, membresía y asignación', async () => {
    const { servicio, pg, client, auth } = crear();
    const vigente = { id: 'i-1', empresa_id: 'e-1', correo: 'ana@core.test', nombre: 'Ana', tipo: 'invitacion', rol_empresa_id: 'r-1', rol_nombre: 'Estilista', tienda_id: 't-1', puesto_id: 'p-1', metadata: { turno: 'manana' }, expira_en: new Date(Date.now() + 60_000).toISOString(), estado: 'pendiente', empresa_nombre: 'Salón' };
    pg.queryOne.mockResolvedValue(vigente);
    await expect(servicio.consultarInvitacion('token')).resolves.toMatchObject({ valida: true, rol_nombre: 'Estilista' });
    client.query.mockImplementation(async (sql: string) => {
      if (sql.includes('FROM identity.invitaciones_personal')) return { rows: [vigente], rowCount: 1 };
      if (sql.includes('FROM identity.usuarios')) return { rows: [], rowCount: 0 };
      if (sql.includes('INSERT INTO identity.usuarios')) return { rows: [{ id: 'u-new', nombre: 'Ana', correo: 'ana@core.test', rol: 'operativo', tenant_id: 'e-1', creado_en: '2026-11-08T00:00:00Z' }], rowCount: 1 };
      return { rows: [{}], rowCount: 1 };
    });
    const sesion = await servicio.aceptarInvitacion('token', { nombre: 'Ana', contrasena: 'UnaClave123' });
    expect(sesion.usuario).toMatchObject({ rol_empresa_id: 'r-1', tienda_id: 't-1', puesto_id: 'p-1' });
    expect(auth.crearSesion).toHaveBeenCalledWith(expect.objectContaining({ rol_empresa_id: 'r-1' }));
  });

  it('rechaza aceptar una invitación inválida o un correo ya registrado', async () => {
    const { servicio, client } = crear();
    await expect(servicio.aceptarInvitacion('token', { nombre: '', contrasena: '123' })).rejects.toMatchObject({ codigo: 'NOMBRE_INVALIDO' });
    client.query.mockResolvedValue({ rows: [], rowCount: 0 });
    await expect(servicio.consultarInvitacion('token')).rejects.toMatchObject({ codigo: 'INVITACION_NO_VALIDA' });
  });
});
