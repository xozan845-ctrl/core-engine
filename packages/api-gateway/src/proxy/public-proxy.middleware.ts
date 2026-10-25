import type { Request, Response } from 'express';
import { Readable } from 'stream';
import { Logger } from '@core/shared';

/**
 * Proxy público same-origin para el frontend de Firebase (ADR-29).
 *
 * El navegador llama a `/api-proxy/api/v1/...`; este handler añade el Basic del
 * gate de staging en el servidor y reenvía al gateway interno. El secreto nunca
 * viaja al navegador. Para rutas con sesión se reenvía el Bearer del cliente y
 * el gateway valida el JWT; el Basic solo se usa en login/registro/refresh y
 * cuando no hay Bearer.
 *
 * La ruta es deliberadamente pública: un frontend público no puede guardar un
 * secreto. La autorización real de datos sigue siendo el JWT. El resto de rutas
 * directas del API conserva el gate Basic.
 */
export interface OpcionesProxyPublico {
  basicAuth?: string;
  origenes: string[];
  puerto: number;
  maxBodyBytes?: number;
  timeoutMs?: number;
}

const PREFIJO = '/api-proxy';
const MAX_BODY_BYTES = 1024 * 1024;
const TIMEOUT_MS = 60_000;
const METODOS = new Set(['GET', 'POST', 'PATCH', 'PUT', 'DELETE', 'OPTIONS']);
const RUTA_AUTH = /^\/api\/v1\/auth\/(login|registro|refresh|restablecer-contrasena)$/;
const CABECERAS_PERMITIDAS = 'Content-Type,Authorization,x-request-id';
const METODOS_CORS = 'GET,POST,PATCH,PUT,DELETE,OPTIONS';
const CABECERAS_NO_REENVIABLES = new Set([
  'connection',
  'keep-alive',
  'transfer-encoding',
  'content-length',
  'content-encoding',
  'upgrade',
  'proxy-authenticate',
  'proxy-authorization',
  'te',
  'trailer',
  'www-authenticate',
]);

function valor(req: Request, nombre: string): string | undefined {
  const cabecera = req.headers[nombre];
  return Array.isArray(cabecera) ? cabecera[0] : cabecera;
}

function normalizarOrigen(valorOrigen: string | undefined): string {
  return (valorOrigen ?? '').trim().toLowerCase().replace(/\/$/, '');
}

function permiteOrigen(origenes: string[], origen: string): string | undefined {
  if (origenes.includes('*')) return '*';
  return origen && origenes.includes(origen) ? origen : undefined;
}

export function crearProxyPublico(opciones: OpcionesProxyPublico) {
  const logger = Logger.create('gateway.proxy-publico');
  const basic = opciones.basicAuth
    ? `Basic ${Buffer.from(opciones.basicAuth, 'utf8').toString('base64')}`
    : undefined;
  const maxBody = opciones.maxBodyBytes ?? MAX_BODY_BYTES;
  const timeout = opciones.timeoutMs ?? TIMEOUT_MS;

  return async (req: Request, res: Response): Promise<void> => {
    const ruta = req.url; // app.use(PREFIJO, ...) ya quitó el prefijo
    if (!ruta.startsWith('/api/v1/')) {
      res.status(404).json({
        codigo: 'NO_ENCONTRADO',
        mensaje: `No existe ruta ${req.method} ${PREFIJO}${ruta}.`,
      });
      return;
    }

    const origen = normalizarOrigen(valor(req, 'origin'));
    const permitido = permiteOrigen(opciones.origenes, origen);
    if (permitido) {
      res.setHeader('Access-Control-Allow-Origin', permitido);
      res.setHeader('Vary', 'Origin');
      res.setHeader('Access-Control-Expose-Headers', 'x-request-id');
      if (permitido !== '*') {
        res.setHeader('Access-Control-Allow-Credentials', 'true');
      }
    }

    if (req.method === 'OPTIONS') {
      res.setHeader('Access-Control-Allow-Methods', METODOS_CORS);
      res.setHeader('Access-Control-Allow-Headers', CABECERAS_PERMITIDAS);
      res.status(204).end();
      return;
    }

    if (!METODOS.has(req.method)) {
      res.status(405).json({ codigo: 'METODO_NO_PERMITIDO', mensaje: 'Metodo no permitido.' });
      return;
    }

    let cuerpo: Buffer | undefined;
    if (req.method !== 'GET' && req.method !== 'HEAD') {
      const trozos: Buffer[] = [];
      let total = 0;
      for await (const trozo of req) {
        const buffer = Buffer.isBuffer(trozo) ? trozo : Buffer.from(trozo);
        total += buffer.length;
        if (total > maxBody) {
          res.status(413).json({
            codigo: 'CUERPO_DEMASIADO_GRANDE',
            mensaje: `El cuerpo supera ${maxBody} bytes.`,
          });
          return;
        }
        trozos.push(buffer);
      }
      cuerpo = trozos.length > 0 ? Buffer.concat(trozos) : undefined;
    }

    const entrante = valor(req, 'authorization');
    const esAuth = RUTA_AUTH.test(ruta.split('?')[0]);
    const autorizacion =
      esAuth || !entrante || !/^Bearer\s+\S+/i.test(entrante) ? basic : entrante;

    const headers: Record<string, string> = {
      'x-request-id': valor(req, 'x-request-id') ?? `proxy-${Date.now()}`,
      accept: valor(req, 'accept') ?? 'application/json',
      'content-type': valor(req, 'content-type') ?? 'application/json',
      'x-forwarded-proto': 'https',
    };
    if (autorizacion) headers.authorization = autorizacion;
    if (req.ip) headers['x-forwarded-for'] = req.ip;

    let upstream: globalThis.Response;
    try {
      upstream = await fetch(`http://127.0.0.1:${opciones.puerto}${ruta}`, {
        method: req.method,
        headers,
        body: cuerpo as unknown as RequestInit['body'],
        redirect: 'manual',
        signal: AbortSignal.timeout(timeout),
      });
    } catch (error) {
      const timeoutError =
        error instanceof Error && (error.name === 'TimeoutError' || error.name === 'AbortError');
      logger.error({
        msg: 'proxy publico: fallo al reenviar',
        ruta,
        err: error instanceof Error ? error.message : String(error),
      });
      if (!res.headersSent) {
        res.status(timeoutError ? 504 : 502).json({
          codigo: timeoutError ? 'PROXY_TIMEOUT' : 'PROXY_NO_DISPONIBLE',
          mensaje: 'No se pudo contactar el API.',
        });
      } else {
        res.end();
      }
      return;
    }

    res.status(upstream.status);
    upstream.headers.forEach((valorCabecera, clave) => {
      const k = clave.toLowerCase();
      if (CABECERAS_NO_REENVIABLES.has(k) || k.startsWith('access-control-')) return;
      try {
        res.setHeader(clave, valorCabecera);
      } catch {
        /* cabecera no reenviable */
      }
    });
    if (permitido) res.setHeader('Vary', 'Origin');

    if (!upstream.body) {
      res.end();
      return;
    }

    const stream = Readable.fromWeb(
      upstream.body as unknown as import('stream/web').ReadableStream,
    );
    stream.on('error', (error) => {
      logger.error({
        msg: 'proxy publico: error de streaming',
        ruta,
        err: error instanceof Error ? error.message : String(error),
      });
      if (!res.headersSent) res.status(502);
      res.end();
    });
    stream.pipe(res as unknown as NodeJS.WritableStream);
  };
}
