import { DomainError, ROLES } from '@core/shared';
import { FieldController } from './field.controller';

describe('FieldController', () => {
  let mocks: Record<string, ReturnType<typeof vi.fn>>;
  let controller: FieldController;
  const usuario = { tenant_id: 'tenant-a' } as never;

  beforeEach(() => {
    mocks = {};
    const field = new Proxy(
      {},
      {
        get: (_target, property: string) => {
          mocks[property] ??= vi.fn();
          return mocks[property];
        },
      },
    );
    controller = new FieldController(field as never);
  });

  function delega(method: string, invoke: () => unknown, expected: unknown[]) {
    const result = { method };
    mocks[method] ??= vi.fn();
    mocks[method].mockReturnValue(result);
    expect(invoke()).toBe(result);
    expect(mocks[method]).toHaveBeenCalledWith(...expected);
  }

  it('rechaza peticiones sin tenant antes de llegar al servicio', () => {
    expect(() => controller.listarPersonal({} as never)).toThrowError(DomainError);
    expect(mocks.listarPersonal).not.toHaveBeenCalled();
  });

  it('permite al empresario administrar la flota de su tenant', () => {
    const rolesCrearVehiculo = Reflect.getMetadata(
      'roles_requeridos', FieldController.prototype.crearVehiculo,
    );
    expect(rolesCrearVehiculo).toContain(ROLES.EMPRESARIO);
    expect(rolesCrearVehiculo).toContain(ROLES.SUPER_ADMIN);
    const rolesListarPersonal = Reflect.getMetadata(
      'roles_requeridos', FieldController.prototype.listarPersonal,
    );
    expect(rolesListarPersonal).toContain(ROLES.EMPRESARIO);
    expect(rolesListarPersonal).toContain(ROLES.SUPER_ADMIN);
  });

  it('propaga tenant y payload en personal, clientes y vehículos', () => {
    delega('listarPersonal', () => controller.listarPersonal(usuario), ['tenant-a']);
    delega('obtenerPersonal', () => controller.obtenerPersonal(usuario, 'p1'), ['tenant-a', 'p1']);
    delega(
      'crearPersonal',
      () => controller.crearPersonal(usuario, { nombre: 'Ana' }),
      ['tenant-a', { nombre: 'Ana' }],
    );
    delega(
      'actualizarPersonal',
      () => controller.actualizarPersonal(usuario, 'p1', { estado: 'en_ruta' }),
      ['tenant-a', 'p1', { estado: 'en_ruta' }],
    );
    delega('eliminarPersonal', () => controller.eliminarPersonal(usuario, 'p1'), ['tenant-a', 'p1']);
    delega('ubicacionPersonal', () => controller.ubicacionPersonal(usuario, 'p1'), ['tenant-a', 'p1']);
    delega(
      'actualizarUbicacion',
      () => controller.actualizarUbicacion(usuario, 'p1', { lat: 12, lng: -86 }),
      ['tenant-a', 'p1', { lat: 12, lng: -86 }],
    );

    delega('listarClientes', () => controller.listarClientes(usuario), ['tenant-a']);
    delega('obtenerCliente', () => controller.obtenerCliente(usuario, 'c1'), ['tenant-a', 'c1']);
    delega(
      'crearCliente',
      () => controller.crearCliente(usuario, { nombreCompleto: 'Cliente A' }),
      ['tenant-a', { nombreCompleto: 'Cliente A' }],
    );
    delega(
      'actualizarCliente',
      () => controller.actualizarCliente(usuario, 'c1', { telefono: '8888' }),
      ['tenant-a', 'c1', { telefono: '8888' }],
    );
    delega('eliminarCliente', () => controller.eliminarCliente(usuario, 'c1'), ['tenant-a', 'c1']);

    delega('listarVehiculos', () => controller.listarVehiculos(usuario), ['tenant-a']);
    delega('obtenerVehiculo', () => controller.obtenerVehiculo(usuario, 'v1'), ['tenant-a', 'v1']);
    delega(
      'crearVehiculo',
      () => controller.crearVehiculo(usuario, { placa: 'M123' }),
      ['tenant-a', { placa: 'M123' }],
    );
    delega(
      'actualizarVehiculo',
      () => controller.actualizarVehiculo(usuario, 'v1', { estado: 'mantenimiento' }),
      ['tenant-a', 'v1', { estado: 'mantenimiento' }],
    );
    delega('eliminarVehiculo', () => controller.eliminarVehiculo(usuario, 'v1'), ['tenant-a', 'v1']);
  });

  it('propaga filtros y parámetros de rutas y pedidos', () => {
    delega('listarRutas', () => controller.listarRutas(usuario, 'p1'), ['tenant-a', 'p1']);
    delega('obtenerRuta', () => controller.obtenerRuta(usuario, 'r1'), ['tenant-a', 'r1']);
    delega(
      'crearRuta',
      () => controller.crearRuta(usuario, { nombre: 'Norte' }),
      ['tenant-a', { nombre: 'Norte' }],
    );
    delega(
      'actualizarRuta',
      () => controller.actualizarRuta(usuario, 'r1', { estado: 'en_curso' }),
      ['tenant-a', 'r1', { estado: 'en_curso' }],
    );
    delega('eliminarRuta', () => controller.eliminarRuta(usuario, 'r1'), ['tenant-a', 'r1']);
    delega(
      'agregarParada',
      () => controller.agregarParada(usuario, 'r1', { nombre: 'Parada' }),
      ['tenant-a', 'r1', { nombre: 'Parada' }],
    );
    delega(
      'actualizarParada',
      () => controller.actualizarParada(usuario, 'r1', 'pa1', { nombre: 'Parada' }),
      ['tenant-a', 'r1', 'pa1', { nombre: 'Parada' }],
    );
    delega(
      'asignarRuta',
      () => controller.asignarRuta(usuario, 'r1', { personalIds: ['p1'] }),
      ['tenant-a', 'r1', { personalIds: ['p1'] }],
    );

    delega(
      'listarPedidos',
      () => controller.listarPedidos(usuario, 'pendiente', 'r1', 'c1'),
      ['tenant-a', { estado: 'pendiente', rutaId: 'r1', clienteId: 'c1' }],
    );
    delega('obtenerPedido', () => controller.obtenerPedido(usuario, 'pe1'), ['tenant-a', 'pe1']);
    delega(
      'crearPedido',
      () => controller.crearPedido(usuario, { cliente: 'Ana' }),
      ['tenant-a', { cliente: 'Ana' }],
    );
    delega(
      'actualizarPedido',
      () => controller.actualizarPedido(usuario, 'pe1', { notas: 'Frágil' }),
      ['tenant-a', 'pe1', { notas: 'Frágil' }],
    );
    delega('eliminarPedido', () => controller.eliminarPedido(usuario, 'pe1'), ['tenant-a', 'pe1']);
    delega(
      'cambiarEstadoPedido',
      () => controller.cambiarEstadoPedido(usuario, 'pe1', { estado: 'entregado' }),
      ['tenant-a', 'pe1', { estado: 'entregado' }],
    );
  });

  it('propaga asistencia, incidencias, telemetría y sync offline', () => {
    delega(
      'listarAsistencia',
      () => controller.listarAsistencia(usuario, 'p1', '2026-10-14'),
      ['tenant-a', { personalId: 'p1', fecha: '2026-10-14' }],
    );
    delega(
      'registrarAsistencia',
      () =>
        controller.registrarAsistencia(usuario, {
          personalId: 'p1',
          tipo: 'entrada',
          estadoPuntualidad: 'puntual',
        }),
      ['tenant-a', { personalId: 'p1', tipo: 'entrada', estadoPuntualidad: 'puntual' }],
    );

    delega(
      'listarIncidencias',
      () => controller.listarIncidencias(usuario, 'abierta', 'r1'),
      ['tenant-a', { estado: 'abierta', rutaId: 'r1' }],
    );
    delega('obtenerIncidencia', () => controller.obtenerIncidencia(usuario, 'i1'), ['tenant-a', 'i1']);
    delega(
      'crearIncidencia',
      () => controller.crearIncidencia(usuario, { tipo: 'mecanica', descripcion: 'Falla' }),
      ['tenant-a', { tipo: 'mecanica', descripcion: 'Falla' }],
    );
    delega(
      'actualizarIncidencia',
      () => controller.actualizarIncidencia(usuario, 'i1', { estado: 'resuelta' }),
      ['tenant-a', 'i1', { estado: 'resuelta' }],
    );
    delega('eliminarIncidencia', () => controller.eliminarIncidencia(usuario, 'i1'), ['tenant-a', 'i1']);

    delega(
      'listarTracking',
      () => controller.listarTracking(usuario, 'p1', 'desde', 'hasta'),
      ['tenant-a', { personalId: 'p1', desde: 'desde', hasta: 'hasta' }],
    );
    delega(
      'registrarTracking',
      () =>
        controller.registrarTracking(usuario, {
          registros: [{ personalId: 'p1', latitud: 12, longitud: -86 }],
        }),
      ['tenant-a', [{ personalId: 'p1', latitud: 12, longitud: -86 }]],
    );
    delega(
      'listarVisitas',
      () => controller.listarVisitas(usuario, 'p1', '2026-10-14'),
      ['tenant-a', { personalId: 'p1', fecha: '2026-10-14' }],
    );
    delega(
      'registrarVisita',
      () => controller.registrarVisita(usuario, { personalId: 'p1', tipoActividad: 'entrega' }),
      ['tenant-a', { personalId: 'p1', tipoActividad: 'entrega' }],
    );
    delega(
      'obtenerCumplimiento',
      () => controller.obtenerCumplimiento(usuario, 'r1', '2026-10-14'),
      ['tenant-a', 'r1', '2026-10-14'],
    );
    delega(
      'guardarCumplimiento',
      () =>
        controller.guardarCumplimiento(usuario, {
          rutaId: 'r1',
          fecha: '2026-10-14',
          metricas: { entregas: 8 },
        }),
      ['tenant-a', { rutaId: 'r1', fecha: '2026-10-14', metricas: { entregas: 8 } }],
    );
    delega(
      'sincronizar',
      () => controller.sincronizar(usuario, { operaciones: [{ tipo: 'pedido', payload: {} }] }),
      ['tenant-a', [{ tipo: 'pedido', payload: {} }]],
    );
  });
});
