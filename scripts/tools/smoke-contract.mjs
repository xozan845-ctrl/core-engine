// NestJS responde 201 por defecto para POST; el OpenAPI del corte también lo declara.
export function corteLiquidacionesValido({ status, json }) {
  return status === 201 && Array.isArray(json);
}
