import { DomainError } from '../errors';

/**
 * Contrato compartido del tema de tienda (ADR-39/ADR-42). Lo usan
 * identity-service (empresa y plataforma) y stores-service (tienda) para no
 * duplicar tokens, defaults ni validación.
 */
export const PLANTILLAS = ['clasica', 'minimal', 'boutique', 'mercado'] as const;
export const FUENTES = ['system', 'inter', 'poppins', 'montserrat', 'playfair'] as const;
export const MODOS = ['claro', 'oscuro', 'auto'] as const;

export type Plantilla = (typeof PLANTILLAS)[number];
export type Fuente = (typeof FUENTES)[number];
export type Modo = (typeof MODOS)[number];

export type OrigenPersonalizacion = 'tienda' | 'empresa' | 'plataforma' | 'defecto';

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

const ERROR_INVALIDO = 'PERSONALIZACION_INVALIDA';
const COLOR_HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

export function esPlantilla(valor: unknown): valor is Plantilla {
  return PLANTILLAS.includes(valor as Plantilla);
}

export function esFuente(valor: unknown): valor is Fuente {
  return FUENTES.includes(valor as Fuente);
}

export function esModo(valor: unknown): valor is Modo {
  return MODOS.includes(valor as Modo);
}

/** Devuelve la plantilla válida o null si no hay override. */
export function plantillaDe(valor: unknown): Plantilla | null {
  return esPlantilla(valor) ? valor : null;
}

/**
 * Normaliza un tema parcial leído de BD o de una petición. Solo conserva
 * claves conocidas y valores con el tipo correcto; el resto se ignora.
 */
export function temaParcialDe(valor: unknown): TemaInput {
  if (!valor || typeof valor !== 'object' || Array.isArray(valor)) return {};
  const fila = valor as Record<string, unknown>;
  const parcial: TemaInput = {};

  const color = (clave: keyof Tema): void => {
    const actual = fila[clave];
    if (typeof actual === 'string' && COLOR_HEX.test(actual.trim())) {
      (parcial as Record<string, unknown>)[clave] = actual.trim().toUpperCase();
    }
  };
  color('color_primario');
  color('color_secundario');
  color('color_acento');
  color('color_fondo');
  color('color_texto');

  if (esFuente(fila.fuente)) parcial.fuente = fila.fuente;
  if (esModo(fila.modo)) parcial.modo = fila.modo;
  if (
    typeof fila.radio_bordes === 'number' &&
    Number.isInteger(fila.radio_bordes) &&
    fila.radio_bordes >= 0 &&
    fila.radio_bordes <= 32
  ) {
    parcial.radio_bordes = fila.radio_bordes;
  }
  return parcial;
}

/** Normaliza un update parcial: recorta/uppercase colores sin rellenar defaults. */
export function normalizarTemaParcial(datos: TemaInput): TemaInput {
  const parcial: TemaInput = {};
  const color = (clave: keyof Tema, valor: string | undefined): void => {
    if (valor !== undefined) {
      (parcial as unknown as Record<string, unknown>)[clave] = valor.trim().toUpperCase();
    }
  };
  color('color_primario', datos.color_primario);
  color('color_secundario', datos.color_secundario);
  color('color_acento', datos.color_acento);
  color('color_fondo', datos.color_fondo);
  color('color_texto', datos.color_texto);
  if (datos.fuente !== undefined) parcial.fuente = datos.fuente;
  if (datos.radio_bordes !== undefined) parcial.radio_bordes = datos.radio_bordes;
  if (datos.modo !== undefined) parcial.modo = datos.modo;
  return parcial;
}

/** Mezcla capas de tema de menor a mayor prioridad sobre los defaults. */
export function mezclarTema(...capas: (TemaInput | null | undefined)[]): Tema {
  const resultado: Tema = { ...TEMA_POR_DEFECTO };
  for (const capa of capas) {
    if (!capa) continue;
    for (const clave of Object.keys(TEMA_POR_DEFECTO) as (keyof Tema)[]) {
      const valor = capa[clave];
      if (valor !== undefined && valor !== null) {
        (resultado as unknown as Record<string, unknown>)[clave] = valor;
      }
    }
  }
  return resultado;
}

/** Tema completo a partir de un valor leído de BD. */
export function temaDe(valor: unknown): Tema {
  return mezclarTema(temaParcialDe(valor));
}

/** Mezcla parcial de un update sobre el tema actual (no sobre los defaults). */
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

/** Reglas de dominio que el DTO no puede garantizar por sí solo. */
export function validarTema(tema: Tema): void {
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
  if (!esFuente(tema.fuente)) {
    throw new DomainError(ERROR_INVALIDO, 'La fuente no es válida.');
  }
  if (
    !Number.isInteger(tema.radio_bordes) ||
    tema.radio_bordes < 0 ||
    tema.radio_bordes > 32
  ) {
    throw new DomainError(ERROR_INVALIDO, 'El radio de bordes debe ser un entero entre 0 y 32.');
  }
  if (!esModo(tema.modo)) {
    throw new DomainError(ERROR_INVALIDO, 'El modo de color no es válido.');
  }
}

export interface TemaHerencia {
  plantilla: Plantilla | null;
  tema: TemaInput;
}

export interface PersonalizacionResuelta {
  plantilla: Plantilla;
  tema: Tema;
  origen: {
    plantilla: OrigenPersonalizacion;
    tema: OrigenPersonalizacion;
  };
}

/**
 * Resuelve la herencia: plataforma → empresa → overrides del nivel superior.
 * `overrides` es la capa más específica (empresa o tienda).
 */
export function resolverPersonalizacion(
  overrides: TemaHerencia,
  empresa: TemaHerencia,
  plataforma: TemaHerencia,
  nivelOverrides: OrigenPersonalizacion = 'tienda',
): PersonalizacionResuelta {
  const tema = mezclarTema(plataforma.tema, empresa.tema, overrides.tema);
  const plantilla =
    overrides.plantilla ?? empresa.plantilla ?? plataforma.plantilla ?? 'clasica';

  const origenPlantilla: OrigenPersonalizacion = overrides.plantilla
    ? nivelOverrides
    : empresa.plantilla
      ? 'empresa'
      : plataforma.plantilla
        ? 'plataforma'
        : 'defecto';

  const tieneOverrides = (valor: TemaInput): boolean => Object.keys(valor).length > 0;
  const origenTema: OrigenPersonalizacion = tieneOverrides(overrides.tema)
    ? nivelOverrides
    : tieneOverrides(empresa.tema)
      ? 'empresa'
      : tieneOverrides(plataforma.tema)
        ? 'plataforma'
        : 'defecto';

  return {
    plantilla,
    tema,
    origen: { plantilla: origenPlantilla, tema: origenTema },
  };
}

const FUENTE_CSS: Record<Fuente, string> = {
  system: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
  inter: 'Inter, system-ui, sans-serif',
  poppins: 'Poppins, system-ui, sans-serif',
  montserrat: 'Montserrat, system-ui, sans-serif',
  playfair: '"Playfair Display", Georgia, serif',
};

/** Genera variables CSS a partir del tema resuelto. */
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
