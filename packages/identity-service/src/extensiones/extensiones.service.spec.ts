import type { Mock } from 'vitest';
import { ExtensionesService } from './extensiones.service';

const EMPRESA = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee';
const ADMIN = { user_id: '11111111-2222-3333-4444-555555555555', rol: 'admin' as const };

function crear() {
  const client = {
    query: vi.fn(),
  };
  const pg = {
    query: vi.fn().mockResolvedValue([]),
    queryOne: vi.fn().mockResolvedValue(null),
    transaccion: vi.fn(async (fn: (value: typeof client) => Promise<unknown>) => fn(client)),
  };
  return { servicio: new ExtensionesService(pg as never), pg, client };
}

const esquema = {
  type: 'object',
  required: ['numero'],
  properties: { numero: { type: 'integer', minimum: 1 } },
  additionalProperties: false,
};

describe('ExtensionesService', () => {
  it('crea una extensión versionada y desactiva la anterior', async () => {
    const { servicio, client } = crear();
    client.query
      .mockResolvedValueOnce({ rows: [] })
      .mockResolvedValueOnce({ rows: [{ version: 2 }] })
      .mockResolvedValueOnce({ rows: [] })
      .mockResolvedValueOnce({
        rows: [{
          id: '22222222-3333-4444-5555-666666666666',
          empresa_id: EMPRESA,
          codigo: 'ficha_cliente',
          nombre: 'Ficha de cliente',
          recurso: 'cliente',
          descripcion: '',
          esquema,
          formulario: {},
          reglas: {},
          version: 2,
          activo: true,
          creado_por: ADMIN.user_id,
          creado_en: '2026-11-08T00:00:00.000Z',
          actualizado_en: '2026-11-08T00:00:00.000Z',
        }],
      });

    const extension = await servicio.crear(EMPRESA, {
      codigo: 'ficha_cliente',
      nombre: 'Ficha de cliente',
      recurso: 'cliente',
      esquema,
    }, ADMIN);

    expect(extension.version).toBe(2);
    expect(client.query).toHaveBeenCalledWith(expect.stringContaining('pg_advisory_xact_lock'), [EMPRESA, 'ficha_cliente']);
    expect(client.query).toHaveBeenCalledWith(expect.stringContaining('SET activo = false'), [EMPRESA, 'ficha_cliente']);
  });

  it('valida datos contra la versión activa y devuelve errores estructurados', async () => {
    const { servicio, pg } = crear();
    (pg.queryOne as Mock)
      .mockResolvedValueOnce({ esquema })
      .mockResolvedValueOnce({ esquema });

    await expect(servicio.validar(EMPRESA, 'ficha_cliente', { numero: 4 }, ADMIN))
      .resolves.toEqual({ valido: true, errores: [] });
    const resultado = await servicio.validar(EMPRESA, 'ficha_cliente', { numero: '4' }, ADMIN);
    expect(resultado.valido).toBe(false);
    expect(resultado.errores[0]).toMatchObject({ mensaje: expect.any(String) });
  });

  it('rechaza schemas inseguros o inválidos antes de persistirlos', async () => {
    const { servicio, pg } = crear();
    await expect(servicio.crear(EMPRESA, {
      codigo: 'ficha_cliente',
      nombre: 'Ficha',
      recurso: 'cliente',
      esquema: { $ref: 'https://externo.test/schema.json' },
    }, ADMIN)).rejects.toMatchObject({ codigo: 'ESQUEMA_EXTENSION_INVALIDO' });
    await expect(servicio.crear(EMPRESA, {
      codigo: 'ficha_cliente',
      nombre: 'Ficha',
      recurso: 'cliente',
      esquema: { type: 'wat' },
    }, ADMIN)).rejects.toMatchObject({ codigo: 'ESQUEMA_EXTENSION_INVALIDO' });
    expect(pg.transaccion).not.toHaveBeenCalled();
  });

  it('impide que un empresario opere otra empresa', async () => {
    const { servicio } = crear();
    await expect(servicio.listar(EMPRESA, {
      user_id: ADMIN.user_id,
      rol: 'empresario',
      tenant_id: '99999999-aaaa-bbbb-cccc-dddddddddddd',
    })).rejects.toMatchObject({ codigo: 'ACCESO_DENEGADO' });
  });
});
