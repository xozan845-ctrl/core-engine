import { Injectable, Optional } from '@nestjs/common';
import { Request, Response } from 'express';
import { Logger, TENANT_HEADER, PERSONAL_HEADER } from '@core/shared';
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
      { patron: /^\/api\/v1\/tiendas\//, metodos: ['*'] },
    ],
  },
  {
    // Especifico ANTES del catch-all de orders: las transiciones de estado
    // (PATCH /orders/:id/estado) pertenecen a logistica (Tabla 21).
    nombre: 'logistics',
    url: process.env.LOGISTICS_SERVICE_URL ?? 'http://logistics-service:3005',
    patrones: [
      { patron: /^\/api\/v1\/orders\/[^/]+\/estado/, metodos: ['PATCH', 'POST'] },
      { patron: /^\/api\/v1\/admin\/envios/, metodos: ['*'] },
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
      { patron: /^\/api\/v1\/admin\/reportes/, metodos: ['*'] },
      { patron: /^\/api\/v1\/admin\/liquidaciones/, metodos: ['*'] },
    ],
  },
  {
    nombre: 'finance',
    url: process.env.FINANCE_SERVICE_URL ?? 'http://finance-service:3007',
    patrones: [
      { patron: /^\/api\/v1\/finanzas(\/|$)/, metodos: ['*'] },
      { patron: /^\/api\/v1\/vendedores\/me\/fiscal/, metodos: ['*'] },
    ],
  },
  {
    // field-service: dominio de logistica de campo (app-test).
    nombre: 'field',
    url: process.env.FIELD_SERVICE_URL ?? 'http://field-service:3008',
    patrones: [{ patron: /^\/api\/v1\/field\//, metodos: ['*'] }],
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

  constructor(
    @Optional() auth?: GatewayAuthMiddleware,
    @Optional() destinos?: ServicioDestino[],
    @Optional() politicas?: PoliticaRuta[],
  ) {
    this.auth = auth ?? new GatewayAuthMiddleware();
    this.destinos = destinos ?? DESTINOS;
    this.politicas = politicas ?? POLITICAS_RUTAS;
  }

  async reenviar(req: Request, res: Response): Promise<void> {
    const ruta = req.originalUrl;
    const politica = this.encontrarPolitica(ruta, req.method);

    // 1. autenticacion y roles en el borde
    this.auth.aplicar(req, res, () => undefined, politica);
    // si la politica ya respondio (401/403), no continuar el reenvio
    if (res.headersSent) return;

    // 2. destino
    const destino = this.encontrarDestino(ruta, req.method);
    if (!destino) {
      res.status(404).json({ codigo: 'NO_ENCONTRADO', mensaje: `No existe ruta ${req.method} ${ruta}.` });
      return;
    }

    // 3. reenvío con las cabeceras de contexto del gateway
    try {
      const cuerpo = ['GET', 'HEAD', 'OPTIONS'].includes(req.method.toUpperCase()) ? undefined : await this.leerCuerpo(req);
      // Whitelist: solo contexto de usuario y correlacion; el resto (host,
      // content-length, transfer-encoding, cookie, etc.) lo calcula fetch
      // y evita cabeceras invalidas hacia el destino.
      const headers: Record<string, string> = {
        'x-request-id': (req.headers['x-request-id'] as string) ?? `gw-${Date.now()}`,
        'content-type': 'application/json',
        accept: 'application/json',
      };
      for (const k of ['x-user-id', 'x-user-email', 'x-user-rol', TENANT_HEADER, PERSONAL_HEADER] as const) {
        const v = req.headers[k];
        if (v !== undefined) headers[k] = Array.isArray(v) ? v.join(', ') : String(v);
      }

      const respuesta = await fetch(`${destino.url}${ruta}`, {
        method: req.method,
        headers,
        body: cuerpo ? JSON.stringify(cuerpo) : undefined,
      });

      const texto = await respuesta.text();
      res.status(respuesta.status);
      for (const [k, v] of Object.entries(respuesta.headers)) {
        try {
          res.setHeader(k, v as string);
        } catch {
          /* headers no reenvariables */
        }
      }
      res.send(texto || undefined);
    } catch (err) {
      this.logger.error({
        msg: 'Fallo el reenvio al servicio',
        ruta,
        destino: destino.nombre,
        err: err instanceof Error ? err.message : String(err),
      });
      if (res.headersSent) {
        res.end();
        return;
      }
      res.status(502).json({
        codigo: 'DESTINO_NO_DISPONIBLE',
        mensaje: 'El servicio de destino no respondio (circuit breaker del gateway).',
        detalles: { servicio: destino.nombre },
      });
    }
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