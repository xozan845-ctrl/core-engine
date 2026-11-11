import { ClientesService } from './clientes.service';

function respuesta(cuerpo: unknown, status = 200): Response {
  return {
    status,
    ok: status >= 200 && status < 300,
    json: async () => cuerpo,
  } as unknown as Response;
}

describe('ClientesService', () => {
  beforeEach(() => {
    process.env.INTERNAL_API_KEY = 'clave-test';
    process.env.ORDERS_SERVICE_URL = 'http://orders:3004/';
    process.env.IDENTITY_SERVICE_URL = 'http://identity:3001/';
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    delete process.env.INTERNAL_API_KEY;
    delete process.env.ORDERS_SERVICE_URL;
    delete process.env.IDENTITY_SERVICE_URL;
  });

  it('consulta la orden por la API interna con la clave y sin doble barra', async () => {
    const fetchMock = vi.fn().mockResolvedValue(respuesta({ id: 'o-1' }));
    vi.stubGlobal('fetch', fetchMock);

    await expect(new ClientesService().obtenerOrden('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')).resolves.toEqual({
      id: 'o-1',
    });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe('http://orders:3004/internal/orders/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');
    expect(init.headers['x-internal-key']).toBe('clave-test');
  });

  it('consulta el usuario por identity', async () => {
    const fetchMock = vi.fn().mockResolvedValue(respuesta({ id: 'u-1', correo: 'a@b.c' }));
    vi.stubGlobal('fetch', fetchMock);

    await expect(new ClientesService().obtenerUsuario('11111111-1111-1111-1111-111111111111')).resolves.toMatchObject({
      correo: 'a@b.c',
    });
    expect(fetchMock.mock.calls[0][0]).toBe(
      'http://identity:3001/internal/usuarios/11111111-1111-1111-1111-111111111111',
    );
  });

  it('un 404 devuelve null y un 5xx se propaga para reintentar', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce(respuesta({}, 404)));
    await expect(
      new ClientesService().obtenerOrden('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'),
    ).resolves.toBeNull();

    vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce(respuesta({}, 503)));
    await expect(
      new ClientesService().obtenerOrden('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'),
    ).rejects.toThrow('orders respondió 503');
  });

  it('un fallo de red se propaga para reintentar', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('ECONNREFUSED')));
    await expect(
      new ClientesService().obtenerUsuario('11111111-1111-1111-1111-111111111111'),
    ).rejects.toThrow('No se pudo consultar identity');
  });

  it('no consulta ids que no son UUID', async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);

    await expect(new ClientesService().obtenerOrden('../../etc/passwd')).resolves.toBeNull();
    await expect(new ClientesService().obtenerUsuario('otro')).resolves.toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });
});
