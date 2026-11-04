import { DomainError } from '@core/shared';
import { urlDestinoValida } from './personalizacion-tienda';

/**
 * Contenido del storefront de tienda (ADR-40). Igual que el tema, el backend
 * guarda datos estructurados y validados; el frontend decide cómo renderizar.
 */
export const TIPOS_SECCION = [
  'hero',
  'destacados',
  'categorias',
  'ofertas',
  'sobre_nosotros',
  'politicas',
  'contacto',
  'redes',
] as const;

export const REDES_SOCIALES = [
  'instagram',
  'facebook',
  'tiktok',
  'x',
  'whatsapp',
  'youtube',
  'otra',
] as const;

export type TipoSeccion = (typeof TIPOS_SECCION)[number];
export type RedSocialTipo = (typeof REDES_SOCIALES)[number];

export interface Seccion {
  tipo: TipoSeccion;
  visible: boolean;
  orden: number;
}

export interface RedSocial {
  red: RedSocialTipo;
  url: string;
}

export interface Politicas {
  envio: string;
  devolucion: string;
  privacidad: string;
}

export interface ContenidoInput {
  secciones?: Seccion[] | null;
  sobre_nosotros?: string;
  politicas?: Partial<Politicas>;
  redes?: RedSocial[] | null;
  anuncio_texto?: string;
  anuncio_enlace?: string;
  anuncio_activo?: boolean;
}

export interface ContenidoCampos {
  secciones: Seccion[];
  sobre_nosotros: string;
  politicas: Politicas;
  redes: RedSocial[];
  anuncio_texto: string;
  anuncio_enlace: string;
  anuncio_activo: boolean;
  contenido_actualizado_en: string | null;
}

type ContenidoFila = Partial<Omit<ContenidoCampos, 'secciones' | 'politicas' | 'redes'>> & {
  secciones?: unknown;
  politicas?: unknown;
  redes?: unknown;
};

export const LIMITES_CONTENIDO = {
  secciones: 12,
  sobre_nosotros: 2000,
  politica: 1000,
  redes: 6,
  anuncio_texto: 120,
  anuncio_enlace: 300,
} as const;

export const POLITICAS_POR_DEFECTO: Readonly<Politicas> = Object.freeze({
  envio: '',
  devolucion: '',
  privacidad: '',
});

export const CONTENIDO_POR_DEFECTO: Readonly<ContenidoCampos> = Object.freeze({
  secciones: [],
  sobre_nosotros: '',
  politicas: POLITICAS_POR_DEFECTO,
  redes: [],
  anuncio_texto: '',
  anuncio_enlace: '',
  anuncio_activo: false,
  contenido_actualizado_en: null,
});

const ERROR_INVALIDO = 'CONTENIDO_INVALIDO';

function texto(valor: unknown, respaldo = ''): string {
  return typeof valor === 'string' ? valor.trim() : respaldo;
}

function json(valor: unknown): unknown {
  if (typeof valor !== 'string') return valor;
  try {
    return JSON.parse(valor);
  } catch {
    return null;
  }
}

function leerSecciones(valor: unknown): Seccion[] {
  const parsed = json(valor);
  if (!Array.isArray(parsed)) return [];
  return parsed
    .filter((item): item is Record<string, unknown> => !!item && typeof item === 'object')
    .filter((item) => TIPOS_SECCION.includes(item.tipo as TipoSeccion))
    .map((item) => ({
      tipo: item.tipo as TipoSeccion,
      visible: item.visible !== false,
      orden:
        typeof item.orden === 'number' && Number.isInteger(item.orden) ? item.orden : 0,
    }))
    .sort((a, b) => a.orden - b.orden);
}

function leerPoliticas(valor: unknown): Politicas {
  const parsed = json(valor);
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return { ...POLITICAS_POR_DEFECTO };
  }
  const fila = parsed as Record<string, unknown>;
  return {
    envio: texto(fila.envio),
    devolucion: texto(fila.devolucion),
    privacidad: texto(fila.privacidad),
  };
}

function leerRedes(valor: unknown): RedSocial[] {
  const parsed = json(valor);
  if (!Array.isArray(parsed)) return [];
  return parsed
    .filter((item): item is Record<string, unknown> => !!item && typeof item === 'object')
    .filter((item) => REDES_SOCIALES.includes(item.red as RedSocialTipo))
    .map((item) => ({ red: item.red as RedSocialTipo, url: texto(item.url) }))
    .filter((item) => item.url !== '');
}

/** Completa una fila parcial con los valores por defecto. */
export function contenidoDe(fila?: ContenidoFila | null): ContenidoCampos {
  if (!fila) {
    return {
      ...CONTENIDO_POR_DEFECTO,
      politicas: { ...POLITICAS_POR_DEFECTO },
      secciones: [],
      redes: [],
    };
  }
  return {
    secciones: leerSecciones(fila.secciones),
    sobre_nosotros: texto(fila.sobre_nosotros),
    politicas: leerPoliticas(fila.politicas),
    redes: leerRedes(fila.redes),
    anuncio_texto: texto(fila.anuncio_texto),
    anuncio_enlace: texto(fila.anuncio_enlace),
    anuncio_activo: fila.anuncio_activo === true,
    contenido_actualizado_en: fila.contenido_actualizado_en ?? null,
  };
}

function normalizarSecciones(secciones?: Seccion[] | null): Seccion[] {
  if (!secciones) return [];
  return secciones
    .map((seccion) => ({
      tipo: seccion.tipo,
      visible: seccion.visible,
      orden: seccion.orden,
    }))
    .sort((a, b) => a.orden - b.orden);
}

function normalizarRedes(redes?: RedSocial[] | null): RedSocial[] {
  if (!redes) return [];
  return redes.map((red) => ({ red: red.red, url: red.url.trim() }));
}

/** Mezcla parcial: un campo ausente conserva el valor actual. */
export function normalizarContenido(
  datos: ContenidoInput,
  actual?: ContenidoCampos | null,
): ContenidoCampos {
  const base = actual ?? contenidoDe(null);
  return {
    secciones:
      datos.secciones !== undefined
        ? normalizarSecciones(datos.secciones)
        : base.secciones,
    sobre_nosotros:
      datos.sobre_nosotros !== undefined
        ? datos.sobre_nosotros.trim()
        : base.sobre_nosotros,
    politicas:
      datos.politicas !== undefined
        ? {
            envio:
              datos.politicas.envio !== undefined
                ? datos.politicas.envio.trim()
                : base.politicas.envio,
            devolucion:
              datos.politicas.devolucion !== undefined
                ? datos.politicas.devolucion.trim()
                : base.politicas.devolucion,
            privacidad:
              datos.politicas.privacidad !== undefined
                ? datos.politicas.privacidad.trim()
                : base.politicas.privacidad,
          }
        : base.politicas,
    redes:
      datos.redes !== undefined ? normalizarRedes(datos.redes) : base.redes,
    anuncio_texto:
      datos.anuncio_texto !== undefined
        ? datos.anuncio_texto.trim()
        : base.anuncio_texto,
    anuncio_enlace:
      datos.anuncio_enlace !== undefined
        ? datos.anuncio_enlace.trim()
        : base.anuncio_enlace,
    anuncio_activo:
      datos.anuncio_activo !== undefined
        ? datos.anuncio_activo
        : base.anuncio_activo,
    contenido_actualizado_en: base.contenido_actualizado_en,
  };
}

/** Reglas de dominio que el DTO no puede garantizar por sí solo. */
export function validarContenido(meta: ContenidoCampos): void {
  if (meta.secciones.length > LIMITES_CONTENIDO.secciones) {
    throw new DomainError(
      ERROR_INVALIDO,
      `No se pueden configurar más de ${LIMITES_CONTENIDO.secciones} secciones.`,
    );
  }
  const tipos = new Set<string>();
  const ordenes = new Set<number>();
  for (const seccion of meta.secciones) {
    if (!TIPOS_SECCION.includes(seccion.tipo)) {
      throw new DomainError(ERROR_INVALIDO, `La sección "${seccion.tipo}" no es válida.`);
    }
    if (typeof seccion.visible !== 'boolean') {
      throw new DomainError(ERROR_INVALIDO, 'La visibilidad de una sección debe ser booleana.');
    }
    if (
      !Number.isInteger(seccion.orden) ||
      seccion.orden < 0 ||
      seccion.orden >= LIMITES_CONTENIDO.secciones
    ) {
      throw new DomainError(
        ERROR_INVALIDO,
        `El orden de una sección debe ser un entero entre 0 y ${LIMITES_CONTENIDO.secciones - 1}.`,
      );
    }
    if (tipos.has(seccion.tipo)) {
      throw new DomainError(ERROR_INVALIDO, `La sección "${seccion.tipo}" está repetida.`);
    }
    if (ordenes.has(seccion.orden)) {
      throw new DomainError(ERROR_INVALIDO, `El orden ${seccion.orden} está repetido.`);
    }
    tipos.add(seccion.tipo);
    ordenes.add(seccion.orden);
  }

  if (meta.sobre_nosotros.length > LIMITES_CONTENIDO.sobre_nosotros) {
    throw new DomainError(
      ERROR_INVALIDO,
      `sobre_nosotros supera ${LIMITES_CONTENIDO.sobre_nosotros} caracteres.`,
    );
  }

  const politicas: [string, string][] = [
    ['envio', meta.politicas.envio],
    ['devolucion', meta.politicas.devolucion],
    ['privacidad', meta.politicas.privacidad],
  ];
  for (const [campo, valor] of politicas) {
    if (valor.length > LIMITES_CONTENIDO.politica) {
      throw new DomainError(
        ERROR_INVALIDO,
        `La política de ${campo} supera ${LIMITES_CONTENIDO.politica} caracteres.`,
      );
    }
  }

  if (meta.redes.length > LIMITES_CONTENIDO.redes) {
    throw new DomainError(
      ERROR_INVALIDO,
      `No se pueden configurar más de ${LIMITES_CONTENIDO.redes} redes.`,
    );
  }
  const redes = new Set<string>();
  for (const red of meta.redes) {
    if (!REDES_SOCIALES.includes(red.red)) {
      throw new DomainError(ERROR_INVALIDO, `La red "${red.red}" no es válida.`);
    }
    if (redes.has(red.red)) {
      throw new DomainError(ERROR_INVALIDO, `La red "${red.red}" está repetida.`);
    }
    if (!red.url || !urlDestinoValida(red.url) || !red.url.startsWith('https://')) {
      throw new DomainError(ERROR_INVALIDO, 'Cada red debe tener una URL HTTPS válida.');
    }
    redes.add(red.red);
  }

  if (meta.anuncio_texto.length > LIMITES_CONTENIDO.anuncio_texto) {
    throw new DomainError(
      ERROR_INVALIDO,
      `El anuncio supera ${LIMITES_CONTENIDO.anuncio_texto} caracteres.`,
    );
  }
  if (meta.anuncio_enlace.length > LIMITES_CONTENIDO.anuncio_enlace) {
    throw new DomainError(
      ERROR_INVALIDO,
      `El enlace del anuncio supera ${LIMITES_CONTENIDO.anuncio_enlace} caracteres.`,
    );
  }
  if (!urlDestinoValida(meta.anuncio_enlace)) {
    throw new DomainError(
      ERROR_INVALIDO,
      'El enlace del anuncio debe ser una ruta relativa o una URL HTTPS.',
    );
  }
  if (typeof meta.anuncio_activo !== 'boolean') {
    throw new DomainError(ERROR_INVALIDO, 'El estado del anuncio debe ser booleano.');
  }
}
