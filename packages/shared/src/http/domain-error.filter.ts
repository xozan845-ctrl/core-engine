import { ArgumentsHost, Catch, ExceptionFilter, HttpException } from '@nestjs/common';
import type { Response } from 'express';
import { DomainError, httpStatusDe } from '../errors';
import { Logger } from '../logging/logger';

/** Codigo estructurado por status HTTP (doc 5.7: 400, 401, 403, 404, 409, 429, 500). */
function codigoDeStatus(status: number): string {
  switch (status) {
    case 400:
      return 'SOLICITUD_INVALIDA';
    case 401:
      return 'NO_AUTORIZADO';
    case 403:
      return 'ACCESO_DENEGADO';
    case 404:
      return 'NO_ENCONTRADO';
    case 409:
      return 'CONFLICTO';
    case 429:
      return 'DEMASIADAS_PETICIONES';
    default:
      return `HTTP_${status}`;
  }
}

/**
 * Filtro global de excepciones (doc 5.7): respuestas {codigo, mensaje, detalles}
 * para TODOS los errores. Los DomainError conservan su codigo de dominio; las
 * HttpException de Nest (incluido el 429 de limitacion de tasa) se normalizan;
 * los fallos no controlados devuelven 500 sin filtrar detalles internos.
 */
@Catch()
export class DomainErrorFilter implements ExceptionFilter {
  private readonly logger = Logger.create('http');

  catch(excepcion: unknown, host: ArgumentsHost): void {
    const res = host.switchToHttp().getResponse<Response>();

    if (excepcion instanceof HttpException) {
      const status = excepcion.getStatus();
      const cuerpo = excepcion.getResponse();
      if (typeof cuerpo === 'string') {
        res
          .status(status)
          .json({ codigo: codigoDeStatus(status), mensaje: cuerpo, detalles: undefined });
        return;
      }
      const objeto = cuerpo as { message?: unknown; error?: unknown };
      const detalles =
        typeof objeto.message === 'string' ? objeto.error ?? undefined : objeto.message;
      const mensaje =
        typeof objeto.message === 'string'
          ? objeto.message
          : objeto.error
            ? String(objeto.error)
            : 'Peticion invalida.';
      res.status(status).json({ codigo: codigoDeStatus(status), mensaje, detalles });
      return;
    }

    if (excepcion instanceof DomainError) {
      res.status(httpStatusDe(excepcion)).json(excepcion.toResponse());
      return;
    }

    // Errores de infraestructura HTTP del adaptador (body-parser, routing):
    // PayloadTooLargeError tiene status 413 pero no es HttpException de Nest
    const statusBruto = (excepcion as { status?: unknown })?.status;
    if (typeof statusBruto === 'number' && statusBruto >= 400 && statusBruto < 500) {
      const msg =
        statusBruto === 413
          ? 'El cuerpo de la peticion excede el limite permitido.'
          : 'Peticion invalida.';
      res.status(statusBruto).json({ codigo: codigoDeStatus(statusBruto), mensaje: msg, detalles: undefined });
      return;
    }

    const err = excepcion instanceof Error ? excepcion.message : String(excepcion);
    this.logger.error({ msg: 'Error no controlado', err });
    res.status(500).json({ codigo: 'ERROR_INTERNO', mensaje: 'Error interno del servidor.' });
  }
}