import { DomainError, NotFoundError, esUuid } from '@core/shared';

/**
 * Contrato comun de media background (ADR-38) para tiendas. El binario vive en
 * storage-service; stores solo guarda la referencia y deriva el tipo y las URLs.
 */
export const BACKGROUND_IMAGENES = ['image/jpeg', 'image/png', 'image/webp'] as const;
export const BACKGROUND_VIDEOS = ['video/mp4', 'video/webm'] as const;
export const BACKGROUND_POSICIONES = ['center', 'top', 'bottom', 'left', 'right'] as const;

export type BackgroundMediaType = 'imagen' | 'video';
export type BackgroundPosition = (typeof BACKGROUND_POSICIONES)[number];

export interface BackgroundInput {
  background_objeto_id?: string | null;
  background_poster_objeto_id?: string | null;
  background_alt?: string;
  background_overlay?: number;
  background_position?: BackgroundPosition;
  background_activo?: boolean;
}

export interface BackgroundCampos {
  background_objeto_id: string | null;
  background_media_type: BackgroundMediaType | null;
  background_poster_objeto_id: string | null;
  background_alt: string;
  background_overlay: number;
  background_position: BackgroundPosition;
  background_activo: boolean;
}

export interface BackgroundConUrls extends BackgroundCampos {
  background_url: string | null;
  background_poster_url: string | null;
}

export const BACKGROUND_POR_DEFECTO: Readonly<BackgroundCampos> = Object.freeze({
  background_objeto_id: null,
  background_media_type: null,
  background_poster_objeto_id: null,
  background_alt: '',
  background_overlay: 35,
  background_position: 'center',
  background_activo: false,
});

/**
 * Completa los campos de fondo de una fila parcial. Las filas creadas antes de
 * la migracion (o los dobles de prueba) pueden no traerlos.
 */
export function backgroundDe(fila?: Partial<BackgroundCampos> | null): BackgroundCampos {
  if (!fila) return { ...BACKGROUND_POR_DEFECTO };
  return {
    background_objeto_id: fila.background_objeto_id ?? null,
    background_media_type: fila.background_media_type ?? null,
    background_poster_objeto_id: fila.background_poster_objeto_id ?? null,
    background_alt: fila.background_alt ?? '',
    background_overlay: fila.background_overlay ?? 35,
    background_position: fila.background_position ?? 'center',
    background_activo: fila.background_activo ?? false,
  };
}

interface ObjetoStorage {
  id: string;
  tenant_id: string;
  estado: string;
  publico: boolean;
  content_type: string;
}

const ERROR_INVALIDO = 'BACKGROUND_INVALIDO';
const ERROR_NO_VERIFICABLE = 'BACKGROUND_NO_VERIFICABLE';

/**
 * Mezcla parcial del fondo. `null` en el objeto limpia todo el bloque; un
 * objeto nuevo deja `background_media_type` a `null` hasta que el servicio
 * valide el binario real en storage.
 */
export function normalizarBackground(
  datos: BackgroundInput,
  actual?: BackgroundCampos | null,
): BackgroundCampos {
  const base = actual ?? BACKGROUND_POR_DEFECTO;
  if (datos.background_objeto_id === null) return { ...BACKGROUND_POR_DEFECTO };

  const objetoId =
    datos.background_objeto_id !== undefined
      ? datos.background_objeto_id
      : base.background_objeto_id;
  const mismoObjeto = objetoId === base.background_objeto_id;
  return {
    background_objeto_id: objetoId,
    background_media_type: mismoObjeto ? base.background_media_type : null,
    background_poster_objeto_id:
      datos.background_poster_objeto_id !== undefined
        ? datos.background_poster_objeto_id
        : base.background_poster_objeto_id,
    background_alt:
      datos.background_alt !== undefined ? datos.background_alt.trim() : base.background_alt,
    background_overlay:
      datos.background_overlay !== undefined ? datos.background_overlay : base.background_overlay,
    background_position:
      datos.background_position !== undefined
        ? datos.background_position
        : base.background_position,
    background_activo:
      datos.background_activo !== undefined ? datos.background_activo : base.background_activo,
  };
}

/** Reglas de dominio que el DTO no puede garantizar por si solo. */
export function validarBackground(datos: BackgroundInput, meta: BackgroundCampos): void {
  if (!meta.background_objeto_id) {
    if (datos.background_activo === true) {
      throw new DomainError(ERROR_INVALIDO, 'No se puede activar un fondo sin objeto.');
    }
    if (datos.background_poster_objeto_id) {
      throw new DomainError(ERROR_INVALIDO, 'El poster requiere un objeto de fondo.');
    }
  }
  if (meta.background_objeto_id !== null && !esUuid(meta.background_objeto_id)) {
    throw new DomainError(ERROR_INVALIDO, 'El fondo debe ser un objeto de storage válido.');
  }
  if (meta.background_poster_objeto_id !== null && !esUuid(meta.background_poster_objeto_id)) {
    throw new DomainError(ERROR_INVALIDO, 'El poster debe ser un objeto de storage válido.');
  }
  if (meta.background_alt.length > 160) {
    throw new DomainError(ERROR_INVALIDO, 'El texto alternativo supera 160 caracteres.');
  }
  if (
    !Number.isInteger(meta.background_overlay) ||
    meta.background_overlay < 0 ||
    meta.background_overlay > 100
  ) {
    throw new DomainError(ERROR_INVALIDO, 'El overlay debe ser un entero entre 0 y 100.');
  }
  if (!BACKGROUND_POSICIONES.includes(meta.background_position)) {
    throw new DomainError(ERROR_INVALIDO, 'La posición del fondo no es válida.');
  }
  if (typeof meta.background_activo !== 'boolean') {
    throw new DomainError(ERROR_INVALIDO, 'El estado activo debe ser booleano.');
  }
}

/** Deriva las rutas publicas; nunca se persisten ni se aceptan del cliente. */
export function conBackgroundUrls(campos: BackgroundCampos): BackgroundConUrls {
  return {
    ...campos,
    background_url: campos.background_objeto_id
      ? `/api/v2/storage/public/objetos/${campos.background_objeto_id}`
      : null,
    background_poster_url: campos.background_poster_objeto_id
      ? `/api/v2/storage/public/objetos/${campos.background_poster_objeto_id}`
      : null,
  };
}

/** Variante móvil del fondo (ADR-41): solo objeto, poster, alt y activo. */
export interface BackgroundMovilInput {
  background_movil_objeto_id?: string | null;
  background_movil_poster_objeto_id?: string | null;
  background_movil_alt?: string;
  background_movil_activo?: boolean;
}

export interface BackgroundMovilCampos {
  background_movil_objeto_id: string | null;
  background_movil_media_type: BackgroundMediaType | null;
  background_movil_poster_objeto_id: string | null;
  background_movil_alt: string;
  background_movil_activo: boolean;
}

export interface BackgroundMovilConUrls extends BackgroundMovilCampos {
  background_movil_url: string | null;
  background_movil_poster_url: string | null;
}

export const BACKGROUND_MOVIL_POR_DEFECTO: Readonly<BackgroundMovilCampos> = Object.freeze({
  background_movil_objeto_id: null,
  background_movil_media_type: null,
  background_movil_poster_objeto_id: null,
  background_movil_alt: '',
  background_movil_activo: false,
});

export function backgroundMovilDe(
  fila?: Partial<BackgroundMovilCampos> | null,
): BackgroundMovilCampos {
  if (!fila) return { ...BACKGROUND_MOVIL_POR_DEFECTO };
  return {
    background_movil_objeto_id: fila.background_movil_objeto_id ?? null,
    background_movil_media_type: fila.background_movil_media_type ?? null,
    background_movil_poster_objeto_id: fila.background_movil_poster_objeto_id ?? null,
    background_movil_alt: fila.background_movil_alt ?? '',
    background_movil_activo: fila.background_movil_activo ?? false,
  };
}

export function normalizarBackgroundMovil(
  datos: BackgroundMovilInput,
  actual?: BackgroundMovilCampos | null,
): BackgroundMovilCampos {
  const base = actual ?? BACKGROUND_MOVIL_POR_DEFECTO;
  if (datos.background_movil_objeto_id === null) return { ...BACKGROUND_MOVIL_POR_DEFECTO };

  const objetoId =
    datos.background_movil_objeto_id !== undefined
      ? datos.background_movil_objeto_id
      : base.background_movil_objeto_id;
  const mismoObjeto = objetoId === base.background_movil_objeto_id;
  return {
    background_movil_objeto_id: objetoId,
    background_movil_media_type: mismoObjeto ? base.background_movil_media_type : null,
    background_movil_poster_objeto_id:
      datos.background_movil_poster_objeto_id !== undefined
        ? datos.background_movil_poster_objeto_id
        : base.background_movil_poster_objeto_id,
    background_movil_alt:
      datos.background_movil_alt !== undefined
        ? datos.background_movil_alt.trim()
        : base.background_movil_alt,
    background_movil_activo:
      datos.background_movil_activo !== undefined
        ? datos.background_movil_activo
        : base.background_movil_activo,
  };
}

export function validarBackgroundMovil(
  datos: BackgroundMovilInput,
  meta: BackgroundMovilCampos,
): void {
  if (!meta.background_movil_objeto_id) {
    if (datos.background_movil_activo === true) {
      throw new DomainError(
        ERROR_INVALIDO,
        'No se puede activar un fondo móvil sin objeto.',
      );
    }
    if (datos.background_movil_poster_objeto_id) {
      throw new DomainError(
        ERROR_INVALIDO,
        'El poster móvil requiere un objeto de fondo móvil.',
      );
    }
  }
  if (meta.background_movil_objeto_id !== null && !esUuid(meta.background_movil_objeto_id)) {
    throw new DomainError(
      ERROR_INVALIDO,
      'El fondo móvil debe ser un objeto de storage válido.',
    );
  }
  if (
    meta.background_movil_poster_objeto_id !== null &&
    !esUuid(meta.background_movil_poster_objeto_id)
  ) {
    throw new DomainError(
      ERROR_INVALIDO,
      'El poster móvil debe ser un objeto de storage válido.',
    );
  }
  if (meta.background_movil_alt.length > 160) {
    throw new DomainError(
      ERROR_INVALIDO,
      'El texto alternativo móvil supera 160 caracteres.',
    );
  }
  if (typeof meta.background_movil_activo !== 'boolean') {
    throw new DomainError(ERROR_INVALIDO, 'El estado activo móvil debe ser booleano.');
  }
}

export function conBackgroundMovilUrls(
  campos: BackgroundMovilCampos,
): BackgroundMovilConUrls {
  return {
    ...campos,
    background_movil_url: campos.background_movil_objeto_id
      ? `/api/v2/storage/public/objetos/${campos.background_movil_objeto_id}`
      : null,
    background_movil_poster_url: campos.background_movil_poster_objeto_id
      ? `/api/v2/storage/public/objetos/${campos.background_movil_poster_objeto_id}`
      : null,
  };
}

/**
 * Valida un objeto contra storage-service (metadata interna) y devuelve el
 * tipo derivado. El poster solo admite imagenes.
 */
export class ValidadorMediaBackground {
  constructor(
    private readonly storageUrl: string,
    private readonly claveInterna: string,
  ) {}

  async validar(
    tenantId: string,
    objetoId: string,
    uso: 'fondo' | 'poster' | 'imagen' = 'fondo',
  ): Promise<BackgroundMediaType> {
    let respuesta: Response;
    try {
      respuesta = await fetch(
        `${this.storageUrl}/internal/storage/objetos/${encodeURIComponent(objetoId)}` +
          `?tenant_id=${encodeURIComponent(tenantId)}`,
        {
          headers: { 'x-internal-key': this.claveInterna },
          // R-RB-4: una validacion colgada no debe consumir el request completo.
          signal: AbortSignal.timeout(4000),
        },
      );
    } catch {
      throw new DomainError(ERROR_NO_VERIFICABLE, 'No se pudo verificar la media del fondo.');
    }
    if (respuesta.status === 404) {
      const recurso = uso === 'poster' ? 'Poster' : uso === 'imagen' ? 'Imagen' : 'Fondo';
      throw new NotFoundError(recurso, objetoId);
    }
    if (!respuesta.ok) {
      throw new DomainError(ERROR_NO_VERIFICABLE, 'No se pudo verificar la media del fondo.');
    }

    const objeto = (await respuesta.json()) as ObjetoStorage;
    const permitidos: readonly string[] =
      uso === 'fondo' ? [...BACKGROUND_IMAGENES, ...BACKGROUND_VIDEOS] : BACKGROUND_IMAGENES;
    if (
      objeto.tenant_id !== tenantId ||
      objeto.estado !== 'activo' ||
      objeto.publico !== true ||
      !permitidos.includes(String(objeto.content_type).toLowerCase())
    ) {
      const mensaje =
        uso === 'poster'
          ? 'El poster debe ser una imagen pública y activa del mismo tenant.'
          : uso === 'imagen'
            ? 'La imagen debe ser una imagen pública y activa del mismo tenant.'
            : 'El fondo debe ser una imagen o video público y activo del mismo tenant.';
      throw new DomainError(ERROR_INVALIDO, mensaje);
    }
    return String(objeto.content_type).toLowerCase().startsWith('video/') ? 'video' : 'imagen';
  }
}
