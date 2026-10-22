import { vi } from 'vitest';
import * as cron from 'node-cron';
import { LiquidacionesService } from './liquidaciones.service';

const { logger } = vi.hoisted(() => ({
  logger: { info: vi.fn(), error: vi.fn() },
}));

vi.mock('node-cron', () => ({ schedule: vi.fn() }));
vi.mock('@core/shared', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@core/shared')>();
  return { ...actual, Logger: { create: () => logger } };
});

function crear() {
  const pg = { query: vi.fn(), queryOne: vi.fn() };
  return new LiquidacionesService(pg as never);
}

describe('LiquidacionesService (planificador)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('programa el corte los días 1 y 15 a las 00:05 y registra el arranque', () => {
    const servicio = crear();
    servicio.onModuleInit();

    expect(cron.schedule).toHaveBeenCalledWith(
      '5 0 1,15 * *',
      expect.any(Function),
    );
    expect(logger.info).toHaveBeenCalledWith({
      msg: 'Planificador de liquidacion quincenal activo (dias 1 y 15)',
    });
  });

  it('el callback cierra el periodo anterior y registra el fallo', async () => {
    const servicio = crear();
    const spy = vi
      .spyOn(servicio, 'cerrarPeriodoAnterior')
      .mockRejectedValue(new Error('boom'));
    servicio.onModuleInit();

    const callback = vi.mocked(cron.schedule).mock.calls[0][1] as () => Promise<void>;
    await callback();

    expect(spy).toHaveBeenCalledTimes(1);
    expect(logger.error).toHaveBeenCalledWith({
      msg: 'Fallo el corte quincenal',
      err: 'boom',
    });
  });

  it('registra el resumen del corte ejecutado', async () => {
    const pg = { query: vi.fn(), queryOne: vi.fn() };
    const servicio = new LiquidacionesService(pg as never);
    pg.query
      .mockResolvedValueOnce([
        {
          id: 'c1',
          vendedor_id: 'v1',
          empresa_id: 'e1',
          monto_vendedor_cents: 100,
          monto_empresario_cents: 20,
        },
      ])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([]);
    pg.queryOne.mockResolvedValue({ id: 'l1' });

    await servicio.cerrarPeriodo({ inicio: '2026-08-01', fin: '2026-08-15' });

    expect(logger.info).toHaveBeenCalledWith({
      msg: 'Corte quincenal ejecutado',
      periodo: '2026-08-01..2026-08-15',
      vendedores: 1,
    });
  });
});
