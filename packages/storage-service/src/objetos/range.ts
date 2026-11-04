/**
 * Interpretacion de `Range` para el servido publico de imagenes y videos
 * (RFC 7233). Solo se admite un rango por peticion: los multiparte agregan
 * complejidad sin que el contrato de media background los necesite.
 */
export interface RangoBytes {
  inicio: number;
  fin: number;
}

export type ResultadoRango =
  | { tipo: 'completo' }
  | { tipo: 'parcial'; rango: RangoBytes }
  | { tipo: 'invalido' };

const RANGO = /^bytes=(\d*)-(\d*)$/i;

function longitud(total: number): number {
  return Number.isSafeInteger(total) && total > 0 ? total : 0;
}

/**
 * Devuelve el rango solicitado o `completo` cuando no hay cabecera. Un rango
 * fuera de los limites, invertido o con sintaxis desconocida es `invalido`
 * (el controlador responde 416 y `Content-Range: bytes * /total`).
 */
export function interpretarRango(cabecera: string | undefined, totalBytes: number): ResultadoRango {
  const total = longitud(totalBytes);
  if (cabecera === undefined || cabecera.trim() === '') return { tipo: 'completo' };

  const coincidencia = RANGO.exec(cabecera.trim());
  if (!coincidencia || total === 0) return { tipo: 'invalido' };

  const inicioTexto = coincidencia[1] ?? '';
  const finTexto = coincidencia[2] ?? '';
  if (!inicioTexto && !finTexto) return { tipo: 'invalido' };

  // `bytes=-N`: los ultimos N bytes.
  if (!inicioTexto) {
    const sufijo = Number(finTexto);
    if (!Number.isSafeInteger(sufijo) || sufijo <= 0) return { tipo: 'invalido' };
    return {
      tipo: 'parcial',
      rango: { inicio: Math.max(0, total - sufijo), fin: total - 1 },
    };
  }

  const inicio = Number(inicioTexto);
  if (!Number.isSafeInteger(inicio) || inicio < 0 || inicio >= total) {
    return { tipo: 'invalido' };
  }

  if (!finTexto) {
    return { tipo: 'parcial', rango: { inicio, fin: total - 1 } };
  }

  const fin = Number(finTexto);
  if (!Number.isSafeInteger(fin) || fin < inicio) return { tipo: 'invalido' };
  return { tipo: 'parcial', rango: { inicio, fin: Math.min(fin, total - 1) } };
}
