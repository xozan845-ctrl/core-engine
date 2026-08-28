import {
  Injectable,
  NestInterceptor,
  ExecutionContext,
  CallHandler,
  PipeTransform,
  ArgumentMetadata,
  Optional,
} from '@nestjs/common';
import { Observable } from 'rxjs';
import { tap } from 'rxjs/operators';
import { MetricsService } from '../metrics/metrics.service';
import { Logger, ejecutarConContexto, ContextoCorrelacion } from '../logging/logger';
import { randomUUID } from 'crypto';
import { plainToInstance } from 'class-transformer';
import { validate } from 'class-validator';
import { ValidationError } from '../errors';

/**
 * Interceptor global: ID de correlacion (x-request-id), logs estructurados y
 * metricas de latencia (doc 5.3).
 */
@Injectable()
export class TrazabilidadInterceptor implements NestInterceptor {
  private readonly logger = Logger.create('http');

  constructor(
    private readonly metrics: MetricsService,
    @Optional() private readonly servicio?: string,
  ) {}

  intercept(ctx: ExecutionContext, next: CallHandler): Observable<unknown> {
    const req = ctx.switchToHttp().getRequest();
    const res = ctx.switchToHttp().getResponse();
    const inicio = Date.now();
    const request_id = (req.headers?.['x-request-id'] as string) ?? randomUUID();
    res.setHeader('x-request-id', request_id);
    const contexto: ContextoCorrelacion = {
      request_id,
      servicio: this.servicio ?? process.env.SERVICIO ?? 'servicio',
    };

    // el handler corre dentro del contexto de correlacion: todos los logs que
    // se emitan aguas abajo llevan el request_id (doc 5.3)
    return ejecutarConContexto(contexto, () => {
      this.logger.info({
        msg: 'peticion',
        metodo: req.method,
        ruta: req.originalUrl ?? req.url,
        usuario: req.headers?.['x-user-id'] ?? null,
      });
      return next.handle().pipe(
        tap({
          next: () => undefined,
          error: () => undefined,
          finalize: () => {
            res.on('finish', () => {
              this.metrics.registrarPeticion(
                req.method ?? 'GET',
                req.originalUrl ?? req.url ?? '/',
                res.statusCode ?? 500,
                Date.now() - inicio,
              );
            });
          },
        }),
      );
    }) as Observable<unknown>;
  }
}

/**
 * Respuestas de error: {codigo, mensaje, detalles} (doc 5.7).
 */
export function cuerpoDeError(
  codigo: string,
  mensaje: string,
  detalles?: unknown,
): { codigo: string; mensaje: string; detalles?: unknown } {
  return detalles === undefined ? { codigo, mensaje } : { codigo, mensaje, detalles };
}

/**
 * Pipe de validacion con class-validator para DTOs (control OWASP A03:
 * validacion de entradas con DTOs tipados).
 */
@Injectable()
export class DtoValidationPipe implements PipeTransform {
  async transform(value: unknown, metadata: ArgumentMetadata): Promise<unknown> {
    if (!metadata.metatype || !this.puedeValidar(metadata.metatype)) return value;
    const objeto = plainToInstance(metadata.metatype as new () => object, value);
    const errores = await validate(objeto as object, {
      whitelist: true,
      forbidNonWhitelisted: false,
    });
    if (errores.length > 0) {
      const detalles = errores.map((e) => ({
        campo: e.property,
        restricciones: Object.keys(e.constraints ?? {}),
      }));
      throw new ValidationError(detalles);
    }
    return objeto;
  }

  private puedeValidar(metatype: unknown): boolean {
    const tipos: unknown[] = [String, Boolean, Number, Array, Object];
    return typeof metatype === 'function' && !tipos.includes(metatype);
  }
}