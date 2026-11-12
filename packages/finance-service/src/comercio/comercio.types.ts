/**
 * Recursos de operación de comercio que se persisten en el libro genérico
 * `finance.workspace_registros`. Cada recurso comparte el aislamiento por
 * empresa (`tenant_id`) o por vendedor (`propietario_id`) y un payload JSONB
 * cuyas claves son camelCase para que el contrato público no obligue al
 * frontend a traducir nombres.
 */
export type RecursoComercio =
  | 'bancos-cuentas'
  | 'creditos'
  | 'gastos'
  | 'cuentas-por-pagar'
  | 'proveedores'
  | 'compras'
  | 'ordenes-compra'
  | 'caja-metodos-pago'
  | 'caja-movimientos'
  | 'caja-cierres'
  | 'caja-chica-fondo'
  | 'caja-chica-movimientos'
  | 'caja-transferencias'
  | 'configuracion-documentos'
  | 'clientes'
  | 'codigos-barras';

/** Recursos que solo existen en el workspace de empresa (ADR-66/ADR-72). */
export const RECURSOS_SOLO_EMPRESA: ReadonlySet<RecursoComercio> = new Set([
  'gastos',
  'proveedores',
  'compras',
  'ordenes-compra',
  'caja-metodos-pago',
  'caja-movimientos',
  'caja-cierres',
  'caja-chica-fondo',
  'caja-chica-movimientos',
  'caja-transferencias',
  'configuracion-documentos',
]);

/** Ámbito resuelto a partir del contexto del gateway. */
export interface AmbitoComercio {
  /** Empresa dueña de los datos, o `null` si el ámbito es de un vendedor. */
  tenantId: string | null;
  /** Vendedor dueño de los datos, o `null` si el ámbito es de una empresa. */
  propietarioId: string | null;
}

/** Fila tal como se guarda en `finance.workspace_registros`. */
export interface FilaRegistroComercio {
  id: string;
  datos: Record<string, unknown>;
  creado_en: Date | string;
  actualizado_en: Date | string;
}

/** Registro público: columnas de control + payload del recurso. */
export type RegistroComercio = Record<string, unknown> & {
  id: string;
  createdAt: string;
  updatedAt: string;
};
