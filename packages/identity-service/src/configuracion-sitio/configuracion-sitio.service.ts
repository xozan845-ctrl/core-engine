import { Injectable } from '@nestjs/common';
import { DomainError, PgService, esUuid } from '@core/shared';
import {
  BACKGROUND_POR_DEFECTO,
  BackgroundCampos,
  BackgroundConUrls,
  BackgroundInput,
  ValidadorMediaBackground,
  conBackgroundUrls,
  normalizarBackground,
  validarBackground,
} from '../media-background/media-background';

export interface ConfiguracionSitio extends BackgroundCampos {
  actualizado_en: string | null;
}

export type ActualizarConfiguracionSitioInput = BackgroundInput;

const COLUMNAS = `background_objeto_id, background_media_type, background_poster_objeto_id,
  background_alt, background_overlay, background_position, background_activo, actualizado_en`;

/**
 * Configuracion visible del sitio (branding de plataforma). El fondo se guarda
 * como referencia a storage-service; las URLs se derivan en cada respuesta.
 */
@Injectable()
export class ConfiguracionSitioService {
  private readonly storageUrl = process.env.STORAGE_SERVICE_URL ?? 'http://storage-service:3010';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly tenantPlataforma = (process.env.PLATFORM_TENANT_ID ?? '').trim();
  private readonly validador = new ValidadorMediaBackground(this.storageUrl, this.claveInterna);

  constructor(private readonly pg: PgService) {}

  async obtener(): Promise<BackgroundConUrls & { actualizado_en: string | null }> {
    const fila = await this.pg.queryOne<ConfiguracionSitio>(
      `SELECT ${COLUMNAS} FROM identity.configuracion_plataforma WHERE id = 1`,
    );
    return this.conUrls(fila ?? this.porDefecto());
  }

  async actualizar(
    datos: ActualizarConfiguracionSitioInput,
    usuarioId: string,
  ): Promise<BackgroundConUrls & { actualizado_en: string | null }> {
    const actual = await this.pg.queryOne<ConfiguracionSitio>(
      `SELECT ${COLUMNAS} FROM identity.configuracion_plataforma WHERE id = 1`,
    );
    const meta = normalizarBackground(datos, actual ?? null);
    validarBackground(datos, meta);

    if (
      meta.background_objeto_id &&
      meta.background_objeto_id !== actual?.background_objeto_id
    ) {
      meta.background_media_type = await this.validar(meta.background_objeto_id, 'fondo');
    }
    if (
      meta.background_poster_objeto_id &&
      meta.background_poster_objeto_id !== actual?.background_poster_objeto_id
    ) {
      await this.validar(meta.background_poster_objeto_id, 'poster');
    }

    const fila = await this.pg.queryOne<ConfiguracionSitio>(
      `INSERT INTO identity.configuracion_plataforma
         (id, background_objeto_id, background_media_type, background_poster_objeto_id,
          background_alt, background_overlay, background_position, background_activo,
          actualizado_por, actualizado_en)
       VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8, NOW())
       ON CONFLICT (id) DO UPDATE SET
         background_objeto_id = EXCLUDED.background_objeto_id,
         background_media_type = EXCLUDED.background_media_type,
         background_poster_objeto_id = EXCLUDED.background_poster_objeto_id,
         background_alt = EXCLUDED.background_alt,
         background_overlay = EXCLUDED.background_overlay,
         background_position = EXCLUDED.background_position,
         background_activo = EXCLUDED.background_activo,
         actualizado_por = EXCLUDED.actualizado_por,
         actualizado_en = NOW()
       RETURNING ${COLUMNAS}`,
      [
        meta.background_objeto_id,
        meta.background_media_type,
        meta.background_poster_objeto_id,
        meta.background_alt,
        meta.background_overlay,
        meta.background_position,
        meta.background_activo,
        usuarioId,
      ],
    );
    if (!fila) {
      throw new DomainError(
        'CONFIGURACION_NO_ACTUALIZADA',
        'No se pudo actualizar la configuración del sitio.',
      );
    }
    return this.conUrls(fila);
  }

  private async validar(
    objetoId: string,
    uso: 'fondo' | 'poster',
  ): Promise<'imagen' | 'video'> {
    if (!esUuid(this.tenantPlataforma)) {
      throw new DomainError(
        'PLATFORM_TENANT_ID_NO_CONFIGURADO',
        'La plataforma no tiene configurado el tenant de sus assets.',
      );
    }
    return this.validador.validar(this.tenantPlataforma, objetoId, uso);
  }

  private porDefecto(): ConfiguracionSitio {
    return { ...BACKGROUND_POR_DEFECTO, actualizado_en: null };
  }

  private conUrls(
    fila: ConfiguracionSitio,
  ): BackgroundConUrls & { actualizado_en: string | null } {
    return { ...fila, ...conBackgroundUrls(fila) };
  }
}
