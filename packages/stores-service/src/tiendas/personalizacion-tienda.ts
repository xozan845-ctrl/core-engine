import { DomainError, esUuid } from '@core/shared';
import {
  PLANTILLAS,
  Plantilla,
  TEMA_POR_DEFECTO,
  Tema,
  TemaInput,
  mezclarTema,
  normalizarTema,
  normalizarTemaParcial,
  plantillaDe,
  temaDe,
  temaParcialDe,
  validarTema,
} from '@core/shared';

/**
 * Personalización visual de tienda (ADR-39/ADR-42). El contrato del tema vive
 * en @core/shared para que identity y stores resuelvan la herencia con las
 * mismas reglas. Este archivo conserva hero/SEO y la resolución efectiva.
 */
export {
  FUENTES,
  Fuente,
  MODOS,
  Modo,
  PLANTILLAS,
  Plantilla,
  TEMA_POR_DEFECTO,
  Tema,
  TemaInput,
  generarTemaCss,
  mezclarTema,
  normalizarTema,
  temaDe,
  temaParcialDe,
  validarTema,
} from '@core/shared';

export interface PersonalizacionInput {
  plantilla?: Plantilla | null;
  tema?: TemaInput | null;
  hero_titulo?: string;
  hero_subtitulo?: string;
  hero_cta_texto?: string;
  hero_cta_url?: string;
  seo_titulo?: string;
  seo_descripcion?: string;
  seo_imagen_objeto_id?: string | null;
}

export interface PersonalizacionCampos {
  plantilla: Plantilla;
  tema: Tema;
  hero_titulo: string;
  hero_subtitulo: string;
  hero_cta_texto: string;
  hero_cta_url: string;
  seo_titulo: string;
  seo_descripcion: string;
  seo_imagen_objeto_id: string | null;
  personalizacion_actualizada_en: string | null;
}

export interface PersonalizacionConUrls extends PersonalizacionCampos {
  seo_imagen_url: string | null;
}

type PersonalizacionFila = Partial<Omit<PersonalizacionCampos, 'tema'>> & {
  tema?: unknown;
};

export const PERSONALIZACION_POR_DEFECTO: Readonly<PersonalizacionCampos> = Object.freeze({
  plantilla: 'clasica',
  tema: TEMA_POR_DEFECTO,
  hero_titulo: '',
  hero_subtitulo: '',
  hero_cta_texto: '',
  hero_cta_url: '',
  seo_titulo: '',
  seo_descripcion: '',
  seo_imagen_objeto_id: null,
  personalizacion_actualizada_en: null,
});

export const LIMITES_PERSONALIZACION = {
  hero_titulo: 80,
  hero_subtitulo: 160,
  hero_cta_texto: 40,
  hero_cta_url: 300,
  seo_titulo: 70,
  seo_descripcion: 300,
} as const;

const ERROR_INVALIDO = 'PERSONALIZACION_INVALIDA';

function texto(valor: unknown, respaldo = ''): string {
  return typeof valor === 'string' ? valor.trim() : respaldo;
}

/**
 * Completa una fila parcial con los valores por defecto. Las tiendas creadas
 * antes de la migración (o los dobles de prueba) no traen las columnas.
 */
export function personalizacionDe(
  fila?: PersonalizacionFila | null,
): PersonalizacionCampos {
  if (!fila) {
    return { ...PERSONALIZACION_POR_DEFECTO, tema: { ...TEMA_POR_DEFECTO } };
  }
  return {
    plantilla: PLANTILLAS.includes(fila.plantilla as Plantilla)
      ? (fila.plantilla as Plantilla)
      : PERSONALIZACION_POR_DEFECTO.plantilla,
    tema: temaDe(fila.tema),
    hero_titulo: texto(fila.hero_titulo),
    hero_subtitulo: texto(fila.hero_subtitulo),
    hero_cta_texto: texto(fila.hero_cta_texto),
    hero_cta_url: texto(fila.hero_cta_url),
    seo_titulo: texto(fila.seo_titulo),
    seo_descripcion: texto(fila.seo_descripcion),
    seo_imagen_objeto_id: fila.seo_imagen_objeto_id ?? null,
    personalizacion_actualizada_en: fila.personalizacion_actualizada_en ?? null,
  };
}

/** Mezcla parcial: un campo ausente conserva el valor actual. */
export function normalizarPersonalizacion(
  datos: PersonalizacionInput,
  actual?: PersonalizacionCampos | null,
): PersonalizacionCampos {
  const base = actual ?? personalizacionDe(null);
  return {
    plantilla:
      datos.plantilla !== undefined && datos.plantilla !== null
        ? datos.plantilla
        : base.plantilla,
    tema: normalizarTema(datos.tema, base.tema),
    hero_titulo:
      datos.hero_titulo !== undefined ? datos.hero_titulo.trim() : base.hero_titulo,
    hero_subtitulo:
      datos.hero_subtitulo !== undefined
        ? datos.hero_subtitulo.trim()
        : base.hero_subtitulo,
    hero_cta_texto:
      datos.hero_cta_texto !== undefined
        ? datos.hero_cta_texto.trim()
        : base.hero_cta_texto,
    hero_cta_url:
      datos.hero_cta_url !== undefined ? datos.hero_cta_url.trim() : base.hero_cta_url,
    seo_titulo: datos.seo_titulo !== undefined ? datos.seo_titulo.trim() : base.seo_titulo,
    seo_descripcion:
      datos.seo_descripcion !== undefined
        ? datos.seo_descripcion.trim()
        : base.seo_descripcion,
    seo_imagen_objeto_id:
      datos.seo_imagen_objeto_id !== undefined
        ? datos.seo_imagen_objeto_id
        : base.seo_imagen_objeto_id,
    personalizacion_actualizada_en: base.personalizacion_actualizada_en,
  };
}

/** Overrides crudos de la tienda: null/vacío significa heredar. */
export interface OverridesPersonalizacion {
  plantilla: Plantilla | null;
  tema: TemaInput;
  hero_titulo: string;
  hero_subtitulo: string;
  hero_cta_texto: string;
  hero_cta_url: string;
  seo_titulo: string;
  seo_descripcion: string;
  seo_imagen_objeto_id: string | null;
  personalizacion_actualizada_en: string | null;
}

type OverridesFila = Partial<Omit<OverridesPersonalizacion, 'plantilla' | 'tema'>> & {
  plantilla_override?: unknown;
  tema_override?: unknown;
};

export function overridesDe(fila?: OverridesFila | null): OverridesPersonalizacion {
  if (!fila) {
    return {
      plantilla: null,
      tema: {},
      hero_titulo: '',
      hero_subtitulo: '',
      hero_cta_texto: '',
      hero_cta_url: '',
      seo_titulo: '',
      seo_descripcion: '',
      seo_imagen_objeto_id: null,
      personalizacion_actualizada_en: null,
    };
  }
  return {
    plantilla: plantillaDe(fila.plantilla_override),
    tema: temaParcialDe(fila.tema_override),
    hero_titulo: texto(fila.hero_titulo),
    hero_subtitulo: texto(fila.hero_subtitulo),
    hero_cta_texto: texto(fila.hero_cta_texto),
    hero_cta_url: texto(fila.hero_cta_url),
    seo_titulo: texto(fila.seo_titulo),
    seo_descripcion: texto(fila.seo_descripcion),
    seo_imagen_objeto_id: fila.seo_imagen_objeto_id ?? null,
    personalizacion_actualizada_en: fila.personalizacion_actualizada_en ?? null,
  };
}

/**
 * Mezcla parcial de overrides. `plantilla: null` y `tema: null` limpian el
 * override y devuelven el control a la herencia.
 */
export function normalizarOverrides(
  datos: PersonalizacionInput,
  actual?: OverridesPersonalizacion | null,
): OverridesPersonalizacion {
  const base = actual ?? overridesDe(null);
  return {
    plantilla: datos.plantilla !== undefined ? datos.plantilla : base.plantilla,
    tema:
      datos.tema !== undefined
        ? datos.tema === null
          ? {}
          : { ...base.tema, ...normalizarTemaParcial(datos.tema) }
        : base.tema,
    hero_titulo:
      datos.hero_titulo !== undefined ? datos.hero_titulo.trim() : base.hero_titulo,
    hero_subtitulo:
      datos.hero_subtitulo !== undefined
        ? datos.hero_subtitulo.trim()
        : base.hero_subtitulo,
    hero_cta_texto:
      datos.hero_cta_texto !== undefined
        ? datos.hero_cta_texto.trim()
        : base.hero_cta_texto,
    hero_cta_url:
      datos.hero_cta_url !== undefined ? datos.hero_cta_url.trim() : base.hero_cta_url,
    seo_titulo: datos.seo_titulo !== undefined ? datos.seo_titulo.trim() : base.seo_titulo,
    seo_descripcion:
      datos.seo_descripcion !== undefined
        ? datos.seo_descripcion.trim()
        : base.seo_descripcion,
    seo_imagen_objeto_id:
      datos.seo_imagen_objeto_id !== undefined
        ? datos.seo_imagen_objeto_id
        : base.seo_imagen_objeto_id,
    personalizacion_actualizada_en: base.personalizacion_actualizada_en,
  };
}

export function validarOverrides(meta: OverridesPersonalizacion): void {
  validarPersonalizacion({
    ...PERSONALIZACION_POR_DEFECTO,
    plantilla: meta.plantilla ?? PERSONALIZACION_POR_DEFECTO.plantilla,
    tema: mezclarTema(meta.tema),
    hero_titulo: meta.hero_titulo,
    hero_subtitulo: meta.hero_subtitulo,
    hero_cta_texto: meta.hero_cta_texto,
    hero_cta_url: meta.hero_cta_url,
    seo_titulo: meta.seo_titulo,
    seo_descripcion: meta.seo_descripcion,
    seo_imagen_objeto_id: meta.seo_imagen_objeto_id,
  });
}

/** Solo rutas relativas seguras o HTTPS; nunca javascript:, data: ni //host. */
export function urlDestinoValida(valor: string): boolean {
  if (!valor) return true;
  if (valor.startsWith('/') && !valor.startsWith('//')) return true;
  try {
    return new URL(valor).protocol === 'https:';
  } catch {
    return false;
  }
}

/** Reglas de dominio que el DTO no puede garantizar por sí solo. */
export function validarPersonalizacion(meta: PersonalizacionCampos): void {
  if (!PLANTILLAS.includes(meta.plantilla)) {
    throw new DomainError(ERROR_INVALIDO, 'La plantilla no es válida.');
  }
  validarTema(meta.tema);

  const textos: [string, string, number][] = [
    ['hero_titulo', meta.hero_titulo, LIMITES_PERSONALIZACION.hero_titulo],
    ['hero_subtitulo', meta.hero_subtitulo, LIMITES_PERSONALIZACION.hero_subtitulo],
    ['hero_cta_texto', meta.hero_cta_texto, LIMITES_PERSONALIZACION.hero_cta_texto],
    ['hero_cta_url', meta.hero_cta_url, LIMITES_PERSONALIZACION.hero_cta_url],
    ['seo_titulo', meta.seo_titulo, LIMITES_PERSONALIZACION.seo_titulo],
    ['seo_descripcion', meta.seo_descripcion, LIMITES_PERSONALIZACION.seo_descripcion],
  ];
  for (const [campo, valor, maximo] of textos) {
    if (valor.length > maximo) {
      throw new DomainError(ERROR_INVALIDO, `${campo} supera ${maximo} caracteres.`);
    }
  }
  if (!urlDestinoValida(meta.hero_cta_url)) {
    throw new DomainError(
      ERROR_INVALIDO,
      'hero_cta_url debe ser una ruta relativa o una URL HTTPS.',
    );
  }
  if (meta.seo_imagen_objeto_id !== null && !esUuid(meta.seo_imagen_objeto_id)) {
    throw new DomainError(
      ERROR_INVALIDO,
      'La imagen SEO debe ser un objeto de storage válido.',
    );
  }
}

/** Deriva la URL pública; nunca se persiste ni se acepta del cliente. */
export function conSeoImagenUrl(
  campos: PersonalizacionCampos,
): PersonalizacionConUrls {
  return {
    ...campos,
    seo_imagen_url: campos.seo_imagen_objeto_id
      ? `/api/v2/storage/public/objetos/${campos.seo_imagen_objeto_id}`
      : null,
  };
}
