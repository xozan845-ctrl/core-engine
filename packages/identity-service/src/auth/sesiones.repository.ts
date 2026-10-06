import { Injectable } from '@nestjs/common';
import { PgService } from '@core/shared';

export interface SesionRow {
  id: string;
  usuario_id: string;
  jti: string;
  creada_en: string;
  expira_en: string;
  revocada_en: string | null;
}

/**
 * Persistencia de sesiones (refresh tokens) en `identity.sesiones` (R-MS-2:
 * unico responsable del SQL). Permite revocar el refresh token al cerrar
 * sesion (R-GW-4, ADR-18). Soft delete por `revocada_en` (R-DB-5).
 */
@Injectable()
export class SesionesRepository {
  constructor(private readonly pg: PgService) {}

  /** Registra una sesion recien emitida (login/registro). */
  async crear(usuarioId: string, jti: string, expiraEn: Date): Promise<void> {
    await this.pg.query(
      `INSERT INTO identity.sesiones (id, usuario_id, jti, expira_en)
       VALUES (gen_random_uuid(), $1, $2, $3)`,
      [usuarioId, jti, expiraEn],
    );
  }

  /** Devuelve la sesion si sigue activa (no revocada y no expirada). */
  activaPorJti(jti: string): Promise<SesionRow | null> {
    return this.pg.queryOne<SesionRow>(
      `SELECT id, usuario_id, jti, creada_en, expira_en, revocada_en
       FROM identity.sesiones
       WHERE jti = $1 AND revocada_en IS NULL AND expira_en > NOW()`,
      [jti],
    );
  }

  /** Revoca una sesion concreta del usuario; devuelve las filas afectadas. */
  async revocar(jti: string, usuarioId: string): Promise<number> {
    const filas = await this.pg.query(
      `UPDATE identity.sesiones SET revocada_en = NOW()
       WHERE jti = $1 AND usuario_id = $2 AND revocada_en IS NULL
       RETURNING id`,
      [jti, usuarioId],
    );
    return filas.length;
  }

  /** Revoca todas las sesiones activas del usuario (logout global / cambio de contrasena). */
  async revocarTodas(usuarioId: string): Promise<number> {
    const filas = await this.pg.query(
      `UPDATE identity.sesiones SET revocada_en = NOW()
       WHERE usuario_id = $1 AND revocada_en IS NULL
       RETURNING id`,
      [usuarioId],
    );
    return filas.length;
  }
}
