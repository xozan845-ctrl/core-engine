import {
  CallHandler,
  ExecutionContext,
  Injectable,
  NestInterceptor,
} from '@nestjs/common';
import { map, Observable } from 'rxjs';

export interface MetaRespuesta {
  timestamp: string;
  path: string;
  method: string;
  statusCode: number;
}

export interface RespuestaApi<T> {
  data: T;
  meta: MetaRespuesta;
}

/** Rutas que no usan el envelope: infraestructura, docs y binarios públicos. */
const RUTAS_SIN_ENVELOPE = [
  '/internal',
  '/health',
  '/metrics',
  '/docs',
  '/api/v2/storage/public',
];

export function rutaLlevaEnvelope(ruta: string): boolean {
  return !RUTAS_SIN_ENVELOPE.some((prefijo) => ruta.startsWith(prefijo));
}

export function esRespuestaApi(valor: unknown): valor is RespuestaApi<unknown> {
  if (!valor || typeof valor !== 'object') return false;
  const posible = valor as Record<string, unknown>;
  const meta = posible.meta;
  return (
    'data' in posible &&
    !!meta &&
    typeof meta === 'object' &&
    'timestamp' in (meta as object) &&
    'path' in (meta as object) &&
    'method' in (meta as object) &&
    'statusCode' in (meta as object)
  );
}

export function crearRespuestaApi<T>(data: T, meta: MetaRespuesta): RespuestaApi<T> {
  return { data, meta };
}

/**
 * Envelope de éxito de la API pública (R-C-1): `{ data, meta }`.
 * No envuelve `/internal`, `/health`, `/metrics`, `/docs` ni binarios de storage,
 * y deja intactas las respuestas sin cuerpo (204/undefined).
 */
@Injectable()
export class EnvelopeInterceptor implements NestInterceptor {
  intercept(ctx: ExecutionContext, next: CallHandler): Observable<unknown> {
    if (ctx.getType() !== 'http') return next.handle();
    const req = ctx.switchToHttp().getRequest();
    const res = ctx.switchToHttp().getResponse();
    const ruta = String(req.originalUrl ?? req.url ?? '');
    if (!rutaLlevaEnvelope(ruta) || res.statusCode === 204) return next.handle();

    return next.handle().pipe(
      map((data: unknown) => {
        if (data === undefined || esRespuestaApi(data)) return data;
        return crearRespuestaApi(data, {
          timestamp: new Date().toISOString(),
          path: ruta,
          method: String(req.method ?? 'GET'),
          statusCode: Number(res.statusCode ?? 200),
        });
      }),
    );
  }
}
