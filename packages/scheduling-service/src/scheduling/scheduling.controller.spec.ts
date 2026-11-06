import { describe, expect, it, vi } from 'vitest';
import { SchedulingController } from './scheduling.controller';

const usuario = { tenant_id: '11111111-1111-4111-8111-111111111111', user_id: 'u-1' } as never;

describe('SchedulingController', () => {
  it('delega recursos, disponibilidad, citas y recordatorios al servicio', async () => {
    const scheduling = {
      listarRecursos: vi.fn().mockResolvedValue([]), crearRecurso: vi.fn().mockResolvedValue({}),
      listarDisponibilidad: vi.fn().mockResolvedValue([]), crearDisponibilidad: vi.fn().mockResolvedValue({}),
      crearBloqueo: vi.fn().mockResolvedValue({}), listarCitas: vi.fn().mockResolvedValue([]),
      crearCita: vi.fn().mockResolvedValue({}), cancelarCita: vi.fn().mockResolvedValue({}),
      listarRecordatorios: vi.fn().mockResolvedValue([]),
    };
    const controller = new SchedulingController(scheduling as never);
    await controller.recursos(usuario);
    await controller.crearRecurso(usuario, { tipo: 'sala', nombre: 'Sala' });
    await controller.disponibilidad(usuario, 'r-1');
    await controller.crearDisponibilidad(usuario, { recurso_id: 'r-1', dia_semana: 1, inicia_hora: '08:00', termina_hora: '17:00' });
    await controller.crearBloqueo(usuario, { recurso_id: 'r-1', inicia_en: '2030-01-01T10:00:00Z', termina_en: '2030-01-01T11:00:00Z' });
    await controller.citas(usuario, '2030-01-01T00:00:00Z', undefined);
    await controller.crearCita(usuario, { recurso_id: 'r-1', inicia_en: '2030-01-01T10:00:00Z', termina_en: '2030-01-01T11:00:00Z' });
    await controller.cancelar(usuario, 'c-1', 'cambio');
    await controller.recordatorios(usuario);
    expect(scheduling.listarRecursos).toHaveBeenCalledWith(usuario.tenant_id);
    expect(scheduling.crearBloqueo).toHaveBeenCalledWith(usuario.tenant_id, usuario.user_id, expect.anything());
  });

  it('exige tenant para operar', () => {
    const controller = new SchedulingController({} as never);
    expect(() => controller.recursos({} as never)).toThrowError('Se requiere una empresa activa.');
  });
});
