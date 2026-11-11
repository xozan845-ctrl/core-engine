import type { Mock } from 'vitest';
import 'reflect-metadata';
import { InternalSchedulingController } from './internal-scheduling.controller';
import type { SchedulingService } from './scheduling.service';
import { ClaveInternaGuard } from '@core/shared';

function fabricar(): {
  scheduling: Record<string, Mock>;
  controller: InternalSchedulingController;
} {
  const scheduling = { verificarDisponibilidad: vi.fn().mockResolvedValue(undefined) };
  return {
    scheduling,
    controller: new InternalSchedulingController(scheduling as unknown as SchedulingService),
  };
}

describe('InternalSchedulingController', () => {
  it('verifica la disponibilidad y responde disponible', async () => {
    const { scheduling, controller } = fabricar();
    const dto = {
      empresa_id: 'e-1',
      recurso_id: 'r-1',
      inicia_en: '2026-12-20T15:00:00.000Z',
      termina_en: '2026-12-22T11:00:00.000Z',
    };
    await expect(controller.disponibilidad(dto)).resolves.toEqual({ disponible: true });
    expect(scheduling.verificarDisponibilidad).toHaveBeenCalledWith(
      'e-1',
      'r-1',
      dto.inicia_en,
      dto.termina_en,
    );
  });

  it('protege la ruta con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternalSchedulingController) as unknown[];
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});
