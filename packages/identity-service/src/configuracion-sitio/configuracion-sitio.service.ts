import { Injectable } from '@nestjs/common';
import {
  DomainError,
  OrigenPersonalizacion,
  PLANTILLAS,
  PgService,
  Plantilla,
  Tema,
  TemaInput,
  esUuid,
  mezclarTema,
  normalizarTemaParcial,
  plantillaDe,
  resolverPersonalizacion,
  temaParcialDe,
  validarTema,
} from '@core/shared';
import {
  BACKGROUND_POR_DEFECTO,
  BackgroundCampos,
  BackgroundConUrls,
  BackgroundInput,
  ValidadorMediaBackground,
  backgroundDe,
  conBackgroundUrls,
  normalizarBackground,
  validarBackground,
} from '../media-background/media-background';

export interface ConfiguracionSitio extends BackgroundCampos {
  plantilla: Plantilla;
  tema: Tema;
  origen_personalizacion: {
    plantilla: OrigenPersonalizacion;
    tema: OrigenPersonalizacion;
  };
  actualizado_en: string | null;
}

export interface ActualizarConfiguracionSitioInput extends BackgroundInput {
  plantilla?: Plantilla | null;
  tema?: TemaInput | null;
}

type ConfiguracionSitioFila = Omit<
  ConfiguracionSitio,
  'plantilla' | 'tema' | 'origen_personalizacion'
> & {
  plantilla_override: string | null;
  tema_override: unknown;
};

const COLUMNAS = `background_objeto_id, background_media_type, background_poster_objeto_id,
  background_alt, background_overlay, background_position, background_activo,
  plantilla_override, tema_override, actualizado_en`;

/**
 * Configuracion visible del sitio (branding de plataforma). El fondo se guarda
 * como referencia a storage-service; las URLs y el tema efectivo se derivan en
 * cada respuesta. Los overrides de tema son la base de la herencia (ADR-42).
 */
@Injectable()
export class ConfiguracionSitioService {
  private readonly storageUrl = process.env.STORAGE_SERVICE_URL ?? 'http://storage-service:3010';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly tenantPlataforma = (process.env.PLATFORM_TENANT_ID ?? '').trim();
  private readonly validador = new ValidadorMediaBackground(this.storageUrl, this.claveInterna);

  constructor(private readonly pg: PgService) {}

  async obtener(): Promise<ConfiguracionSitio & BackgroundConUrls> {
    const fila = await this.pg.queryOne<ConfiguracionSitioFila>(
      `SELECT ${COLUMNAS} FROM identity.configuracion_plataforma WHERE id = 1`,
    );
    return this.conUrls(fila ?? this.porDefecto());
  }

  async actualizar(
    datos: ActualizarConfiguracionSitioInput,
    usuarioId: string,
  ): Promise<ConfiguracionSitio & BackgroundConUrls> {
    const actual = await this.pg.queryOne<ConfiguracionSitioFila>(
      `SELECT ${COLUMNAS} FROM identity.configuracion_plataforma WHERE id = 1`,
    );
    const meta = normalizarBackground(datos, actual ?? null);
    validarBackground(datos, meta);

    const overridesActuales = {
      plantilla: plantillaDe(actual?.plantilla_override),
      tema: temaParcialDe(actual?.tema_override),
    };
    const plantillaOverride =
      datos.plantilla !== undefined ? datos.plantilla : overridesActuales.plantilla;
    const temaOverride =
      datos.tema !== undefined
        ? datos.tema === null
          ? {}
          : { ...overridesActuales.tema, ...normalizarTemaParcial(datos.tema) }
        : overridesActuales.tema;
    if (plantillaOverride !== null && !PLANTILLAS.includes(plantillaOverride)) {
      throw new DomainError('CONFIGURACION_INVALIDA', 'La plantilla no es válida.');
    }
    validarTema(mezclarTema(temaOverride));

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

    const fila = await this.pg.queryOne<ConfiguracionSitioFila>(
      `INSERT INTO identity.configuracion_plataforma
         (id, background_objeto_id, background_media_type, background_poster_objeto_id,
          background_alt, background_overlay, background_position, background_activo,
          plantilla_override, tema_override, actualizado_por, actualizado_en)
       VALUES (1, $1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10, NOW())
       ON CONFLICT (id) DO UPDATE SET
         background_objeto_id = EXCLUDED.background_objeto_id,
         background_media_type = EXCLUDED.background_media_type,
         background_poster_objeto_id = EXCLUDED.background_poster_objeto_id,
         background_alt = EXCLUDED.background_alt,
         background_overlay = EXCLUDED.background_overlay,
         background_position = EXCLUDED.background_position,
         background_activo = EXCLUDED.background_activo,
         plantilla_override = EXCLUDED.plantilla_override,
         tema_override = EXCLUDED.tema_override,
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
        plantillaOverride,
        JSON.stringify(temaOverride),
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

  /** Overrides crudos para que stores resuelva la herencia (ADR-42). */
  async obtenerTemaOverrides(): Promise<{
    plantilla_override: Plantilla | null;
    tema_override: TemaInput;
  }> {
    const fila = await this.pg.queryOne<ConfiguracionSitioFila>(
      `SELECT ${COLUMNAS} FROM identity.configuracion_plataforma WHERE id = 1`,
    );
    return {
      plantilla_override: plantillaDe(fila?.plantilla_override),
      tema_override: temaParcialDe(fila?.tema_override),
    };
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

  private porDefecto(): ConfiguracionSitioFila {
    return {
      ...BACKGROUND_POR_DEFECTO,
      plantilla_override: null,
      tema_override: {},
      actualizado_en: null,
    };
  }

  private conUrls(fila: ConfiguracionSitioFila): ConfiguracionSitio & BackgroundConUrls {
    const { plantilla_override: _plantilla, tema_override: _tema, ...resto } = fila;
    const overrides = {
      plantilla: plantillaDe(fila.plantilla_override),
      tema: temaParcialDe(fila.tema_override),
    };
    const resuelta = resolverPersonalizacion(
      overrides,
      { plantilla: null, tema: {} },
      { plantilla: null, tema: {} },
      'plataforma',
    );
    return {
      ...resto,
      ...conBackgroundUrls(backgroundDe(fila)),
      plantilla: resuelta.plantilla,
      tema: resuelta.tema,
      origen_personalizacion: resuelta.origen,
    };
  }
}
