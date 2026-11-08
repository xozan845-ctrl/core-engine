import { Injectable, Optional } from '@nestjs/common';
import { Request, Response } from 'express';
import { Readable } from 'stream';
import { Logger, TENANT_HEADER, PERSONAL_HEADER, PARTICIPACION_HEADER } from '@core/shared';
import { GatewayAuthMiddleware, PoliticaRuta, POLITICAS_RUTAS } from './gateway-auth.middleware';

export interface ServicioDestino {
  nombre: string;
  url: string;
  patrones: { patron: RegExp; metodos: string[] }[];
}

/**
 * Tabla de enrutado del gateway hacia los microservicios (Tabla 21).
 */
export const DESTINOS: ServicioDestino[] = [
  {
    nombre: 'identity',
    url: process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001',
    patrones: [
      { patron: /^\/api\/v1\/auth\//, metodos: ['*'] },
      { patron: /^\/api\/v1\/usuarios(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/empresas(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/packs(\/|$)/, metodos: ['GET'] },
      { patron: /^\/api\/v1\/invitaciones(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/configuracion(\/|$)/, metodos: ['*'] },
    ],
  },
  {
    nombre: 'storage',
    url: process.env.STORAGE_SERVICE_URL ?? 'http://storage-service:3010',
    patrones: [
      { patron: /^\/api\/v1\/storage(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/admin\/storage(\/|$)/, metodos: ['*'] },
    ],
  },
  {
    nombre: 'catalog',
    url: process.env.CATALOG_SERVICE_URL ?? 'http://catalog-service:3002',
    patrones: [
      { patron: /^\/api\/v1\/catalog\//, metodos: ['*'] },
      { patron: /^\/api\/v1\/admin\/inventario(\/|$)/, metodos: ['*'] },
    ],
  },
  {
    nombre: 'stores',
    url: process.env.STORES_SERVICE_URL ?? 'http://stores-service:3003',
    patrones: [
      { patron: /^\/api\/v1\/vendedores\/(productos|tienda|me\/(ofertas|tienda))/, metodos: ['*'] },
      { patron: /^\/api\/v1\/empresarios\/me\/tiendas$/, metodos: ['GET'] },
      { patron: /^\/api\/v1\/tiendas\//, metodos: ['*'] },
    ],
  },
  {
    nombre: 'scheduling',
    url: process.env.SCHEDULING_SERVICE_URL ?? 'http://scheduling-service:3011',
    patrones: [{ patron: /^\/api\/v1\/scheduling(\/|$)/, metodos: ['*'] }],
  },
  {
    // Especifico ANTES del catch-all de orders: las transiciones de estado
    // (PATCH /orders/:id/estado) pertenecen a logistica (Tabla 21).
    nombre: 'logistics',
    url: process.env.LOGISTICS_SERVICE_URL ?? 'http://logistics-service:3005',
    patrones: [
      { patron: /^\/api\/v1\/orders\/[^/]+\/estado/, metodos: ['PATCH', 'POST'] },
      { patron: /^\/api\/v1\/admin\/envios/, metodos: ['*'] },
      { patron: /^\/api\/v1\/empresarios\/me\/envios/, metodos: ['*'] },
      { patron: /^\/api\/v1\/envios\/[^/]+\/estado$/, metodos: ['PATCH'] },
    ],
  },
  {
    nombre: 'orders',
    url: process.env.ORDERS_SERVICE_URL ?? 'http://orders-service:3004',
    patrones: [
      { patron: /^\/api\/v1\/orders(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/carrito(\/|$)/, metodos: ['*'] },
    ],
  },
  {
    nombre: 'commissions',
    url: process.env.COMMISSIONS_SERVICE_URL ?? 'http://commissions-service:3006',
    patrones: [
      { patron: /^\/api\/v1\/vendedores\/me\/(ventas|liquidaciones)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/empresarios\/me\/(liquidaciones|ventas)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/admin\/reportes/, metodos: ['*'] },
      { patron: /^\/api\/v1\/admin\/liquidaciones/, metodos: ['*'] },
    ],
  },
  {
    nombre: 'finance',
    url: process.env.FINANCE_SERVICE_URL ?? 'http://finance-service:3007',
    patrones: [
      { patron: /^\/api\/v1\/finanzas(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/comercio(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/vendedores\/me\/fiscal/, metodos: ['*'] },
    ],
  },
  {
    // field-service: dominio de logistica de campo (app-test).
    nombre: 'field',
    url: process.env.FIELD_SERVICE_URL ?? 'http://field-service:3008',
    patrones: [{ patron: /^\/api\/v1\/field\//, metodos: ['*'] }],
  },
  {
    nombre: 'intelligence',
    url: process.env.INTELLIGENCE_SERVICE_URL ?? 'http://market-intelligence-service:3009',
    patrones: [
      { patron: /^\/api\/v1\/inteligencia(\/|$)/, metodos: ['*'] },
    ],
  },
];

/**
 * Proxy HTTP: reenvia la peticion al servicio correspondiente, aplica la
 * politica de acceso y devuelve el cuerpo/errores estructurados (doc 5.7).
 */
@Injectable()
export class ProxyService {
  private readonly logger = Logger.create('gateway.proxy');
  private readonly auth: GatewayAuthMiddleware;
  private readonly destinos: ServicioDestino[];
  private readonly politicas: PoliticaRuta[];
  private readonly timeoutMs: number;

  constructor(
    @Optional() auth?: GatewayAuthMiddleware,
    @Optional() destinos?: ServicioDestino[],
    @Optional() politicas?: PoliticaRuta[],
    @Optional() timeoutMs?: number,
  ) {
    this.auth = auth ?? new GatewayAuthMiddleware();
    this.destinos = destinos ?? DESTINOS;
    this.politicas = politicas ?? POLITICAS_RUTAS;
    // R-RB-4: el gateway no puede esperar indefinidamente a un microservicio.
    // 30 s queda por debajo de los 60 s del proxy publico para que el cliente
    // reciba el servicio culpable en vez de un 504 generico.
    const configurado = timeoutMs ?? Number(process.env.PROXY_UPSTREAM_TIMEOUT_MS ?? 30_000);
    this.timeoutMs =
      Number.isFinite(configurado) && configurado > 0 ? Math.trunc(configurado) : 30_000;
  }

  async reenviar(req: Request, res: Response): Promise<void> {
    const ruta = req.originalUrl;
    // El enrutado y las politicas se declaran sobre el path; la query solo se
    // conserva para el reenvio. Sin esto, una regex anclada (`...$`) deja de
    // coincidir cuando el cliente agrega filtros y la peticion cae en el
    // deny-by-default (R-GW-1 / R-GW-3).
    const rutaBase = this.rutaSinQuery(ruta);
    const politica = this.encontrarPolitica(rutaBase, req.method);

    // Scrub defensivo: eliminar cualquier cabecera de identidad o interna
    // que el cliente externo pudiera haber inyectado (R-GW-3 / R-DS-5).
    // Se usan las constantes reales: la cabecera de tenant es `x-tenant`
    // (TENANT_HEADER) y la de personal `x-user-personal` (PERSONAL_HEADER),
    // que son las que leen los servicios; borrar nombres inexistentes no
    // protegia contra el spoofing de tenant.
    for (const h of ['x-user-id', 'x-user-email', 'x-user-rol', 'x-user-nombre',
      TENANT_HEADER, PERSONAL_HEADER, PARTICIPACION_HEADER, 'x-internal-key']) {
      delete req.headers[h];
    }

    // 1. autenticacion y roles en el borde
    this.auth.aplicar(req, res, () => undefined, politica);
    // si la politica ya respondio (401/403), no continuar el reenvio
    if (res.headersSent) return;

    // 2. destino
    const destino = this.encontrarDestino(rutaBase, req.method);
    if (!destino) {
      res.status(404).json({ codigo: 'NO_ENCONTRADO', mensaje: `No existe ruta ${req.method} ${rutaBase}.` });
      return;
    }

    // 3. reenvío con las cabeceras de contexto del gateway
    // Solo se limita la espera de la respuesta (cabeceras). El cuerpo en
    // streaming (imágenes públicas) sigue su curso sin abortarse por lentitud
    // del cliente: el temporizador se limpia al recibir la respuesta.
    let agoto = false;
    try {
      const cuerpo = ['GET', 'HEAD', 'OPTIONS'].includes(req.method.toUpperCase()) ? undefined : await this.leerCuerpo(req);
      // Whitelist: solo contexto de usuario y correlacion; el resto (host,
      // content-length, transfer-encoding, cookie, etc.) lo calcula fetch
      // y evita cabeceras invalidas hacia el destino.
      // Las imágenes y videos públicos son binarios: no se debe pedir JSON ni
      // decodificar el cuerpo como texto (ADR-28). `Range` viaja tal cual para
      // que storage-service pueda responder 206/416.
      const esMediaPublica = /^\/api\/v1\/storage\/public\/objetos\/[^/]+$/.test(rutaBase);
      const headers: Record<string, string> = {
        'x-request-id': (req.headers['x-request-id'] as string) ?? `gw-${Date.now()}`,
        'content-type': esMediaPublica ? 'application/octet-stream' : 'application/json',
        accept: esMediaPublica ? 'image/*,video/*,*/*' : 'application/json',
      };
      for (const k of ['x-user-id', 'x-user-email', 'x-user-rol', TENANT_HEADER, PERSONAL_HEADER, PARTICIPACION_HEADER] as const) {
        const v = req.headers[k];
        if (v !== undefined) headers[k] = Array.isArray(v) ? v.join(', ') : String(v);
      }
      const rango = req.headers.range;
      if (esMediaPublica && rango !== undefined) {
        headers.range = Array.isArray(rango) ? rango.join(', ') : String(rango);
      }

      const controlador = new AbortController();
      const temporizador = setTimeout(() => {
        agoto = true;
        controlador.abort();
      }, this.timeoutMs);
      let respuesta: globalThis.Response;
      try {
        respuesta = await fetch(`${destino.url}${ruta}`, {
          method: req.method,
          headers,
          body: cuerpo ? JSON.stringify(cuerpo) : undefined,
          // Los 301 de slug (ADR-26) deben llegar al cliente con su Location;
          // si fetch los sigue, el cliente recibe 200 y pierde la redireccion.
          redirect: 'manual',
          signal: controlador.signal,
        });
      } finally {
        clearTimeout(temporizador);
      }

      res.status(respuesta.status);
      // `fetch` devuelve un objeto `Headers` (iterable), no un objeto plano:
      // Object.entries() no ve ninguna cabecera y se perderia, entre otras,
      // la Location de los 301 por slug (ADR-26). Las cabeceras hop-by-hop y
      // content-length las gestiona Express al enviar el cuerpo.
      const noReenviables = new Set([
        'connection', 'keep-alive', 'transfer-encoding', 'upgrade',
        'proxy-authenticate', 'proxy-authorization', 'te', 'trailer', 'content-length',
      ]);
      respuesta.headers.forEach((valor, clave) => {
        if (noReenviables.has(clave.toLowerCase())) return;
        try {
          res.setHeader(clave, valor);
        } catch {
          /* headers no reenvariables */
        }
      });
      // Streaming binario: `text()` corrompería una imagen o un video. El body
      // de fetch es un ReadableStream web; se puentea al response de Express.
      const contentType = respuesta.headers.get('content-type') ?? '';
      const esBinario =
        contentType.startsWith('image/') ||
        contentType.startsWith('video/') ||
        contentType.startsWith('application/octet-stream');
      // `content-length` se excluye del reenvío genérico porque Express lo
      // gestiona; para media binaria sí importa (206 y reproducción de video).
      const longitud = respuesta.headers.get('content-length');
      if (esBinario && longitud) res.setHeader('Content-Length', longitud);
      if (esBinario) {
        if (respuesta.body) {
          const stream = Readable.fromWeb(
            respuesta.body as unknown as import('stream/web').ReadableStream,
          );
          stream.on('error', () => {
            if (!res.headersSent) res.status(502);
            res.end();
          });
          stream.pipe(res as unknown as NodeJS.WritableStream);
        } else {
          res.end();
        }
        return;
      }

      const texto = await respuesta.text();
      res.send(texto || undefined);
    } catch (err) {
      this.logger.error({
        msg: agoto ? 'Timeout esperando al servicio' : 'Fallo el reenvio al servicio',
        ruta,
        destino: destino.nombre,
        ...(agoto ? { timeout_ms: this.timeoutMs } : {}),
        err: err instanceof Error ? err.message : String(err),
      });
      if (res.headersSent) {
        res.end();
        return;
      }
      if (agoto) {
        res.status(504).json({
          codigo: 'DESTINO_TIMEOUT',
          mensaje: `El servicio de destino no respondio en ${this.timeoutMs} ms.`,
          detalles: { servicio: destino.nombre, timeout_ms: this.timeoutMs },
        });
        return;
      }
      res.status(502).json({
        codigo: 'DESTINO_NO_DISPONIBLE',
        mensaje: 'El servicio de destino no respondio (circuit breaker del gateway).',
        detalles: { servicio: destino.nombre },
      });
    }
  }

  /** Path de la peticion sin query ni fragmento, para enrutado y politicas. */
  private rutaSinQuery(originalUrl: string): string {
    const corte = originalUrl.search(/[?#]/);
    return corte === -1 ? originalUrl : originalUrl.slice(0, corte);
  }

  private encontrarPolitica(ruta: string, metodo: string): PoliticaRuta | null {
    const encontrada = this.politicas.find(
      (p) => p.patron.test(ruta) && (p.metodos.includes(metodo.toUpperCase()) || p.metodos.includes('*')),
    );
    if (encontrada && encontrada.roles === null) {
      // rutas publicas especificas; las demas exigen token (se valida en aplicar)
      return encontrada;
    }
    return encontrada ?? null;
  }

  private encontrarDestino(ruta: string, metodo: string): ServicioDestino | null {
    for (const destino of this.destinos) {
      const coincide = destino.patrones.some(
        (p) => p.patron.test(ruta) && (p.metodos.includes(metodo.toUpperCase()) || p.metodos.includes('*')),
      );
      if (coincide) return destino;
    }
    return null;
  }

  private async leerCuerpo(req: Request): Promise<unknown> {
    // Nest 11 registra express.json()/urlencoded por defecto: el stream ya fue
    // consumido y req.body trae el objeto; el fallback cubre casos sin parser.
    if (req.body !== undefined) return req.body;
    return new Promise((resolve) => {
      const chunks: Buffer[] = [];
      req.on('data', (c) => chunks.push(Buffer.from(c)));
      req.on('end', () => {
        const texto = Buffer.concat(chunks).toString('utf-8');
        try {
          resolve(texto ? JSON.parse(texto) : undefined);
        } catch {
          resolve(texto);
        }
      });
      req.on('error', () => resolve(undefined));
    });
  }
}
