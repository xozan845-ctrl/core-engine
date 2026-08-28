import { LiquidacionesService } from '../liquidaciones/liquidaciones.service';

describe('LiquidacionesService (RN-07: corte quincenal dias 1 y 15)', () => {
  const servicio = (() => {
    const fake = {
      pg: { query: jest.fn(), queryOne: jest.fn() },
      logger: { info: jest.fn(), error: jest.fn() },
    };
    return new LiquidacionesService(fake.pg as never);
  })();

  it('el dia 10 pertenece a la primera quincena (1-15)', () => {
    const periodo = servicio.periodoDe(new Date(2026, 7, 10)); // 10 agosto 2026
    expect(periodo.inicio).toBe('2026-08-01');
    expect(periodo.fin).toBe('2026-08-15');
  });

  it('el dia 16 pertenece a la segunda quincena (16-fin de mes)', () => {
    const periodo = servicio.periodoDe(new Date(2026, 7, 20)); // 20 agosto 2026
    expect(periodo.inicio).toBe('2026-08-16');
    expect(periodo.fin).toBe('2026-08-31');
  });

  it('febrero: la segunda quincena cierra el dia 28 (no bisiesto)', () => {
    const periodo = servicio.periodoDe(new Date(2026, 1, 25)); // 25 feb 2026
    expect(periodo.fin).toBe('2026-02-28');
  });

  it('el corte del dia 1 cierra el periodo previo del mes anterior', () => {
    const periodo = servicio.periodoACerrar(new Date(2026, 7, 1)); // 1 agosto 2026
    expect(periodo.inicio).toBe('2026-07-16');
    expect(periodo.fin).toBe('2026-07-31');
  });

  it('el corte del dia 15 cierra la primera quincena vigente', () => {
    const periodo = servicio.periodoACerrar(new Date(2026, 7, 15)); // 15 agosto 2026
    expect(periodo.inicio).toBe('2026-08-01');
    expect(periodo.fin).toBe('2026-08-15');
  });
});