import type { Mock } from 'vitest';
import { SuscripcionesScheduler } from './suscripciones.scheduler';
import type { SuscripcionesService } from './suscripciones.service';

describe('SuscripcionesScheduler', () => {
  it('delega la renovacion de vencidas', async () => {
    const suscripciones = { renovarVencidas: vi.fn().mockResolvedValue(2) };
    const scheduler = new SuscripcionesScheduler(
      suscripciones as unknown as SuscripcionesService,
    );
    await scheduler.renovarVencidas();
    expect((suscripciones.renovarVencidas as Mock)).toHaveBeenCalledTimes(1);
  });
});
