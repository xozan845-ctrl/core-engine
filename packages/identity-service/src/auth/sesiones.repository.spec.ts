import { SesionesRepository } from './sesiones.repository';
import type { PgService } from '@core/shared';

/**
 * R-GW-4 / ADR-18: el refresh solo renueva si la sesion sigue activa; el
 * logout la revoca (soft delete). El repositorio oculta el SQL (R-MS-2).
 */
describe('SesionesRepository (identity.sesiones)', () => {
  function fabricar() {
    const pg = {
      query: jest.fn().mockResolvedValue([]),
      queryOne: jest.fn().mockResolvedValue(null),
    } as unknown as PgService;
    const repo = new SesionesRepository(pg);
    return { repo, pg: pg as unknown as { query: jest.Mock; queryOne: jest.Mock } };
  }

  it('debe insertar la sesion con usuario, jti y expiracion', async () => {
    const { repo, pg } = fabricar();
    const expira = new Date('2026-10-13T00:00:00.000Z');
    await repo.crear('u-1', 'jti-1', expira);
    expect(pg.query).toHaveBeenCalledWith(
      expect.stringContaining('INSERT INTO identity.sesiones'),
      ['u-1', 'jti-1', expira],
    );
  });

  it('debe devolver la sesion solo si esta activa (no revocada ni expirada)', async () => {
    const { repo, pg } = fabricar();
    const fila = { id: 's-1', usuario_id: 'u-1', jti: 'jti-1', creada_en: '', expira_en: '', revocada_en: null };
    pg.queryOne.mockResolvedValue(fila);
    await expect(repo.activaPorJti('jti-1')).resolves.toEqual(fila);
    const [sql, params] = pg.queryOne.mock.calls[0];
    expect(sql).toContain('revocada_en IS NULL');
    expect(sql).toContain('expira_en > NOW()');
    expect(params).toEqual(['jti-1']);
  });

  it('debe devolver null cuando la sesion fue revocada o no existe', async () => {
    const { repo, pg } = fabricar();
    pg.queryOne.mockResolvedValue(null);
    await expect(repo.activaPorJti('jti-x')).resolves.toBeNull();
  });

  it('debe reportar cuantas sesiones revoco (una concreta)', async () => {
    const { repo, pg } = fabricar();
    pg.query.mockResolvedValue([{ id: 's-1' }]);
    await expect(repo.revocar('jti-1', 'u-1')).resolves.toBe(1);
    const [sql, params] = pg.query.mock.calls[0];
    expect(sql).toContain('UPDATE identity.sesiones SET revocada_en = NOW()');
    expect(sql).toContain('jti = $1 AND usuario_id = $2');
    expect(params).toEqual(['jti-1', 'u-1']);
  });

  it('debe reportar cero cuando la sesion ya estaba revocada', async () => {
    const { repo, pg } = fabricar();
    pg.query.mockResolvedValue([]);
    await expect(repo.revocar('jti-1', 'u-1')).resolves.toBe(0);
  });

  it('debe revocar todas las sesiones activas del usuario', async () => {
    const { repo, pg } = fabricar();
    pg.query.mockResolvedValue([{ id: 's-1' }, { id: 's-2' }]);
    await expect(repo.revocarTodas('u-1')).resolves.toBe(2);
    expect(pg.query.mock.calls[0][1]).toEqual(['u-1']);
  });
});
