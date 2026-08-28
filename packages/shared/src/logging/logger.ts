import { AsyncLocalStorage } from 'async_hooks';

export interface ContextoCorrelacion {
  request_id: string;
  servicio: string;
}

const almacen = new AsyncLocalStorage<ContextoCorrelacion>();

/** Correlacion distribuida (doc 5.3: trazas con ID de correlacion). */
export function ejecutarConContexto(contexto: ContextoCorrelacion, fn: () => unknown): unknown {
  return almacen.run(contexto, fn);
}

export function contextoActual(): ContextoCorrelacion | undefined {
  return almacen.getStore();
}

/**
 * Logs estructurados en JSON (doc: logs estructurados sin datos sensibles,
 * correlacion por pedido/usuario).
 */
export class Logger {
  private constructor(private readonly servicio: string) {}

  static create(servicio: string): Logger {
    return new Logger(servicio);
  }

  private entrada(nivel: string, campos: Record<string, unknown>): void {
    const ctx = contextoActual();
    const linea = {
      nivel,
      ts: new Date().toISOString(),
      servicio: this.servicio,
      request_id: ctx?.request_id,
      ...campos,
    };
    const texto = JSON.stringify(linea);
    if (nivel === 'error') console.error(texto);
    else if (nivel === 'warn') console.warn(texto);
    else console.log(texto);
  }

  info(campos: Record<string, unknown>): void {
    this.entrada('info', campos);
  }
  warn(campos: Record<string, unknown>): void {
    this.entrada('warn', campos);
  }
  error(campos: Record<string, unknown>): void {
    this.entrada('error', campos);
  }
}