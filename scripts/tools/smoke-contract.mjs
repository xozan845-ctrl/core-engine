// NestJS responde 201 por defecto para POST; el OpenAPI del corte también lo declara.
// El envelope R-C-1 puede entregar el array dentro de `data`.
export function corteLiquidacionesValido({ status, json }) {
  const cuerpo = json && typeof json === 'object' && 'data' in json ? json.data : json;
  return status === 201 && Array.isArray(cuerpo);
}
