import { PgService } from './pg.service';
import { ejecutarConContexto } from '../logging/logger';

const contexto = {
  request_id: 'req-1',
  servicio: 'test',
  usuario_id: 'u-1',
  rol: 'empresario',
  tenant_id: 't-1',
};

function crearCliente() {
  const consultas: Array<{ text: string; params?: unknown[] }> = [];
  return {
    consultas,
    query: vi.fn(async (text: string, params?: unknown[]) => {
      consultas.push({ text, params });
      return { rows: [{ ok: 1 }] };
    }),
    release: vi.fn(),
  };
}

function crearPool(cliente: ReturnType<typeof crearCliente>) {
  return {
    connect: vi.fn().mockResolvedValue(cliente),
    query: vi.fn().mockResolvedValue({ rows: [{ ok: 1 }] }),
    end: vi.fn(),
  };
}

function servicioCon(cliente: ReturnType<typeof crearCliente>) {
  const servicio = new PgService();
  (servicio as unknown as { pool: unknown }).pool = crearPool(cliente);
  return servicio;
}

describe('PgService (RLS y claims de sesión)', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it('con RLS activo y usuario, aplica SET ROLE authenticated y los claims', async () => {
    vi.stubEnv('DB_RLS_ENABLED', 'true');
    const cliente = crearCliente();
    const servicio = servicioCon(cliente);

    await ejecutarConContexto(contexto, () => servicio.query('SELECT 1'));

    const textos = cliente.consultas.map((c) => c.text);
    expect(textos).toContain('SET LOCAL ROLE authenticated');
    const claims = cliente.consultas.find((c) => c.text.includes("set_config('request.jwt.claims'"));
    expect(claims).toBeDefined();
    expect(JSON.parse(String(claims?.params?.[0]))).toMatchObject({
      sub: 'u-1',
      rol: 'empresario',
      tenant_id: 't-1',
    });
  });

  it('con RLS activo y sin usuario, usa el rol de servicio (background)', async () => {
    vi.stubEnv('DB_RLS_ENABLED', 'true');
    const cliente = crearCliente();
    const servicio = servicioCon(cliente);

    await servicio.query('SELECT 1');

    expect(cliente.consultas.map((c) => c.text)).toContain('SET LOCAL ROLE core_engine_service');
  });

  it('con RLS desactivado y usuario, conserva los claims sin cambiar de rol', async () => {
    vi.stubEnv('DB_RLS_ENABLED', 'false');
    const cliente = crearCliente();
    const servicio = servicioCon(cliente);

    await ejecutarConContexto(contexto, () => servicio.query('SELECT 1'));

    const textos = cliente.consultas.map((c) => c.text);
    expect(textos.some((t) => t.startsWith('SET LOCAL ROLE'))).toBe(false);
    expect(textos.some((t) => t.includes("set_config('request.jwt.claims'"))).toBe(true);
  });

  it('con RLS desactivado y sin usuario, consulta sin transacción ni rol', async () => {
    vi.stubEnv('DB_RLS_ENABLED', 'false');
    const cliente = crearCliente();
    const servicio = servicioCon(cliente);
    const pool = (servicio as unknown as { pool: ReturnType<typeof crearPool> }).pool;

    await servicio.query('SELECT 1');

    expect(pool.connect).not.toHaveBeenCalled();
    expect(pool.query).toHaveBeenCalledWith('SELECT 1', []);
  });

  it('rechaza un DB_SERVICE_ROLE que no sea un identificador simple', () => {
    vi.stubEnv('DB_SERVICE_ROLE', 'core_engine_service; DROP TABLE identity.usuarios');
    expect(() => new PgService()).toThrow(/DB_SERVICE_ROLE invalido/);
  });
});
