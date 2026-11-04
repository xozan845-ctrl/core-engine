import { DomainError, esUuid } from '@core/shared';

/**
 * Personalización visual de tienda (ADR-39). El backend guarda tokens
 * validados (plantilla, tema, hero y SEO) y el frontend los renderiza. Nunca
 * se aceptan CSS, HTML ni JavaScript libres: solo valores de una allowlist.
 */
export const PLANTILLAS = ['clasica', 'minimal', 'boutique', 'mercado'] as const;
export const FUENTES = ['system', 'inter', 'poppins', 'montserrat', 'playfair'] as const;
export const MODOS = ['claro', 'oscuro', 'auto'] as const;

export type Plantilla = (typeof PLANTILLAS)[number];
export type Fuente = (typeof FUENTES)[number];
export type Modo = (typeof MODOS)[number];

export interface Tema {
  color_primario: string;
  color_secundario: string;
  color_acento: string;
  color_fondo: string;
  color_texto: string;
  fuente: Fuente;
  radio_bordes: number;
  modo: Modo;
}

export type TemaInput = Partial<Tema>;

export interface PersonalizacionInput {
  plantilla?: Plantilla;
  tema?: TemaInput;
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

export const TEMA_POR_DEFECTO: Readonly<Tema> = Object.freeze({
  color_primario: '#1F2937',
  color_secundario: '#F59E0B',
  color_acento: '#10B981',
  color_fondo: '#FFFFFF',
  color_texto: '#111827',
  fuente: 'system',
  radio_bordes: 12,
  modo: 'claro',
});

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
const COLOR_HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

function texto(valor: unknown, respaldo = ''): string {
  return typeof valor === 'string' ? valor.trim() : respaldo;
}

function color(valor: unknown, respaldo: string): string {
  return typeof valor === 'string' && COLOR_HEX.test(valor.trim())
    ? valor.trim().toUpperCase()
    : respaldo;
}

function temaDe(valor: unknown): Tema {
  const base = TEMA_POR_DEFECTO;
  if (!valor || typeof valor !== 'object' || Array.isArray(valor)) return { ...base };
  const fila = valor as Record<string, unknown>;
  const fuente = FUENTES.includes(fila.fuente as Fuente) ? (fila.fuente as Fuente) : base.fuente;
  const modo = MODOS.includes(fila.modo as Modo) ? (fila.modo as Modo) : base.modo;
  const radio =
    typeof fila.radio_bordes === 'number' && Number.isInteger(fila.radio_bordes)
      ? fila.radio_bordes
      : base.radio_bordes;
  return {
    color_primario: color(fila.color_primario, base.color_primario),
    color_secundario: color(fila.color_secundario, base.color_secundario),
    color_acento: color(fila.color_acento, base.color_acento),
    color_fondo: color(fila.color_fondo, base.color_fondo),
    color_texto: color(fila.color_texto, base.color_texto),
    fuente,
    radio_bordes: radio,
    modo,
  };
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
export function normalizarTema(datos?: TemaInput | null, actual?: Tema | null): Tema {
  const base = actual ?? TEMA_POR_DEFECTO;
  if (!datos) return { ...base };
  return {
    color_primario:
      datos.color_primario !== undefined
        ? datos.color_primario.trim().toUpperCase()
        : base.color_primario,
    color_secundario:
      datos.color_secundario !== undefined
        ? datos.color_secundario.trim().toUpperCase()
        : base.color_secundario,
    color_acento:
      datos.color_acento !== undefined
        ? datos.color_acento.trim().toUpperCase()
        : base.color_acento,
    color_fondo:
      datos.color_fondo !== undefined
        ? datos.color_fondo.trim().toUpperCase()
        : base.color_fondo,
    color_texto:
      datos.color_texto !== undefined
        ? datos.color_texto.trim().toUpperCase()
        : base.color_texto,
    fuente: datos.fuente !== undefined ? datos.fuente : base.fuente,
    radio_bordes:
      datos.radio_bordes !== undefined ? datos.radio_bordes : base.radio_bordes,
    modo: datos.modo !== undefined ? datos.modo : base.modo,
  };
}

export function normalizarPersonalizacion(
  datos: PersonalizacionInput,
  actual?: PersonalizacionCampos | null,
): PersonalizacionCampos {
  const base = actual ?? personalizacionDe(null);
  return {
    plantilla: datos.plantilla !== undefined ? datos.plantilla : base.plantilla,
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

function validarTema(tema: Tema): void {
  const colores: [string, string][] = [
    ['color_primario', tema.color_primario],
    ['color_secundario', tema.color_secundario],
    ['color_acento', tema.color_acento],
    ['color_fondo', tema.color_fondo],
    ['color_texto', tema.color_texto],
  ];
  for (const [campo, valor] of colores) {
    if (!COLOR_HEX.test(valor)) {
      throw new DomainError(
        ERROR_INVALIDO,
        `${campo} debe ser un color hexadecimal (#RGB, #RRGGBB o #RRGGBBAA).`,
      );
    }
  }
  if (!FUENTES.includes(tema.fuente)) {
    throw new DomainError(ERROR_INVALIDO, 'La fuente no es válida.');
  }
  if (
    !Number.isInteger(tema.radio_bordes) ||
    tema.radio_bordes < 0 ||
    tema.radio_bordes > 32
  ) {
    throw new DomainError(ERROR_INVALIDO, 'El radio de bordes debe ser un entero entre 0 y 32.');
  }
  if (!MODOS.includes(tema.modo)) {
    throw new DomainError(ERROR_INVALIDO, 'El modo de color no es válido.');
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
      ? `/api/v1/storage/public/objetos/${campos.seo_imagen_objeto_id}`
      : null,
  };
}

const FUENTE_CSS: Record<Fuente, string> = {
  system: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
  inter: 'Inter, system-ui, sans-serif',
  poppins: 'Poppins, system-ui, sans-serif',
  montserrat: 'Montserrat, system-ui, sans-serif',
  playfair: '"Playfair Display", Georgia, serif',
};

/**
 * Genera variables CSS a partir del tema resuelto. Es una comodidad para el
 * storefront: el frontend puede aplicarlas directamente sin duplicar tokens.
 */
export function generarTemaCss(tema: Tema): string {
  const modo =
    tema.modo === 'auto' ? 'light dark' : tema.modo === 'oscuro' ? 'dark' : 'light';
  const variables: [string, string][] = [
    ['--tienda-color-primario', tema.color_primario],
    ['--tienda-color-secundario', tema.color_secundario],
    ['--tienda-color-acento', tema.color_acento],
    ['--tienda-color-fondo', tema.color_fondo],
    ['--tienda-color-texto', tema.color_texto],
    ['--tienda-fuente', FUENTE_CSS[tema.fuente] ?? FUENTE_CSS.system],
    ['--tienda-radio-bordes', `${tema.radio_bordes}px`],
    ['--tienda-modo', tema.modo],
    ['color-scheme', modo],
  ];
  const cuerpo = variables
    .map(([nombre, valor]) => `  ${nombre}: ${valor};`)
    .join('\n');
  return `:root {\n${cuerpo}\n}\n`;
}
