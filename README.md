# BodegaHub · Core Engine

Plataforma de comercio electronico distribuido (microservicios sobre Node.js/NestJS + TypeScript)
conforme al documento **BodegaHub_Informe.docx v1.6**: CQRS + Event Sourcing en las ordenes,
RabbitMQ con outbox y DLQ, Postgres/Supabase-compatible con esquema por dominio, API Gateway
con autenticacion en el borde, observabilidad (Prometheus + Grafana) y suite de pruebas.

Requiere **Node >= 20 (uso v24)** y **Docker with Compose v2**. Todas las rutas van por el
gateway (`http://localhost:8080/api/v1/...`). Los montos se manejan como **enteros en centavos**
(control OWASP A02): el JSON entrega strings con dos decimales (`"1150.00"`).

## Arquitectura

```
                    ┌─────────────┐   TLS/CORS/Throttling en el borde
  Cliente ─────────▶│ API Gateway │  JWT HS256 (access 15 min + refresh 7 d)
                    │   :8080     │  Politicas por ruta/rol (Tabla 21)
                    └──────┬──────┘
        ┌─────────┬────────┼────────┬──────────┬───────────┬──────────┐
   identity    catalog  stores    orders   logistics  commissions  finance
      :3001       :3002    :3003     :3004     :3005        :3006       :3007
        └─────────┴────────┴────────┴──────────┴───────────┴──────────┘
               RabbitMQ · topic bodegahub.events (+ DLQ, AD-04) + Postgres 16
```

- **CQRS + Event Sourcing** solo en `orders` (alta tasa de escritura, cap. 3.2); el resto usa
  CRUD convencional sobre su propio esquema (baja tasa de escritura).
- **Outbox pattern** (ADR-03): cada servicio publica sus eventos en `x.outbox` en la misma
  transaccion; un reenviador los mueve al broker. RabbitMQ confirma publicacion (confirm channel).
- **Saga** (ADR-02): `order.created` → catalog descuenta stock (atomico) → `stock.reservado` →
  orders marca `pagada` y emite `payment.procesado` → logistics crea el envio → ...
  `stock.fallido` revierte la orden a `cancelada`.
- **DLQ con backoff exponencial** (AD-04): reintentos hasta `REINTENTOS_MAXIMOS`; luego el
  mensaje se declara muerto. `POST /api/v1/admin/...dlq...` (endpoint interno) permite reinyectar.
- **Ciclo de vida de la orden** (Tabla 13): `creada → pagada → en_preparacion → enviada →
  entregada` (y `cancelada` / `devuelta`); las transiciones se validan con `validarTransicion`.

## Reglas de negocio implementadas

| Regla | Descripcion |
|---|---|
| RN-01 | Precio final = precio base × (1 + margen), margen entero 0–90 % |
| RN-02 | No se publica oferta con stock < 1 |
| RN-03 | Descuento atomico de stock; rechazo total si alguna linea no alcanza (409 rápido) |
| RN-04 | Comision 12 % (configurable); el vendedor recibe venta − comision |
| RN-05 | Carrito de 30 min en cliente; no reserva stock; cantidades repetidas se agrupan |
| RN-06 | Devolucion revierte stock y compensa la comision en la siguiente liquidacion |
| RN-07 | Liquidaciones quincenales (dias 1 y 15, cron America/Managua) |
| RN-08 | Cambios de precio base van al historico; aplican solo a ofertas nuevas |

Eventos del bus (topic `bodegahub.events`): `order.created`, `stock.reservado`,
`stock.fallido`, `stock.reintegrado`, `payment.procesado`, `order.completado`,
`comision.acreditada`, `devolucion.solicitada`, `shipment.started`, `stock.updated`,
`order.status.updated`, `declaracion.generada`.

## Puesta en marcha

> El proyecto vive en un disco **NTFS**: los workspaces de npm crean junctions a los paquetes
> (`packages/*` → `node_modules/@core/*`). En discos exFAT/FAT32 (sin soporte de enlaces) `npm install`
> falla con `EISDIR`; mueva el repositorio a NTFS o use `pnpm` con `node-linker=hoisted`.

```bash
npm install                       # instala los workspaces (enlaza @core/shared, ...)
npm run build                     # compila shared + los 8 microservicios
npm test                          # suite: shared, gateway, orders, commissions, finance
cp .env.example .env              # ajuste las claves/URLs si es necesario
docker compose up -d --build      # Postgres + RabbitMQ + 8 servicios + Prometheus + Grafana
npm run demo                      # ejercicio end-to-end (TC-01..TC-08, RN-01..RN-08)
```

Postgres siembra automáticamente `infra/db/init/01_esquemas.sql` (esquemas, tablas, checks,
seeds del plan contable NIC e indices). En **Supabase** (staging/prod) aplique ademas
`infra/db/init/99_rls.sql`, que activa el aislamiento por rol de Tabla 15 (funciones
`auth.rol()`/`auth.uid()`).

### Servicios

| Servicio | Puerto | Esquema | Fuente |
|---|---|---|---|
| api-gateway | 8080 | — | `packages/api-gateway` |
| identity-service | 3001 | `identity` | `packages/identity-service` |
| catalog-service | 3002 | `catalog` | `packages/catalog-service` |
| stores-service | 3003 | `stores` | `packages/stores-service` |
| orders-service | 3004 | `orders` | `packages/orders-service` |
| logistics-service | 3005 | `logistics` | `packages/logistics-service` |
| commissions-service | 3006 | `commissions` | `packages/commissions-service` |
| finance-service | 3007 | `finance` | `packages/finance-service` |
| postgres | 5432 | todos | `infra/db/init/` |
| rabbitmq (+ management) | 5672 / 15672 | — | — |
| prometheus | 9090 | — | `infra/prometheus/` |
| grafana | 3000 | — | `infra/grafana/` |

## API (resumen de Tabla 21, todas por el gateway)

- `POST /api/v1/auth/registro|login|refresh`, `GET /api/v1/auth/me`
- `POST /api/v1/catalog/productos`, `GET /api/v1/catalog/productos(/:id)` (admin)
- `POST /api/v1/vendedores/tienda`, `POST|PATCH /api/v1/vendedores/productos(/:id)`,
  `GET /api/v1/vendedores/me/ofertas`, `GET /api/v1/tiendas/:id` (publica)
- `POST /api/v1/orders`, `GET /api/v1/orders(/:id)` (comprador/vendedor/admin)
- `PATCH /api/v1/orders/:id/estado` (admin: en_preparacion/enviada/entregada/cancelada/devuelta)
- `GET /api/v1/vendedores/me/ventas|liquidaciones`, `GET /api/v1/admin/reportes`,
  `POST /api/v1/admin/liquidaciones/corte`, `POST /api/v1/admin/liquidaciones/:id/pagar`
- `GET /api/v1/admin/inventario`, `GET /api/v1/admin/envios`
- `GET /api/v1/finanzas/asientos|proyecciones|kpis|punto-equilibrio` (admin),
  `POST /api/v1/finanzas/asientos`, `GET /api/v1/vendedores/me/fiscal`
- Infra: `GET /health` (agregada por servicio), `GET /metrics`, Swagger en `/docs`

Endpoints internos (servicio→servicio) protegidos con `x-internal-key`: `internal/usuarios/:id`,
`internal/vendedores`, `internal/productos/lote`, `internal/productos/sku/:sku`,
`internal/ofertas`, `internal/orders(/:id|transicion|reproyectar|historia)`, `internal/envios`,
`internal/liquidaciones/*`.

## Variables de entorno (`.env` → `.env.example`)

`DATABASE_URL`/`POSTGRES_*`, `RABBITMQ_URL`, `JWT_SECRET` (+ TTLs), `INTERNAL_API_KEY`,
`ADMIN_EMAIL/ADMIN_PASSWORD`, `COMMISSION_RATE`, `OUTBOX_TABLA` (por servicio, ej.
`orders.outbox`), `*_SERVICE_URL` (URLs internas), `CORS_ORIGINS`, `LOG_LEVEL`.

## Pruebas

`sandbox` de reglas (Tabla 5) y casos de aceptacion cubiertos: TC-01 alta de producto,
TC-02 oferta con margen, TC-03 checkout con descuento de stock, TC-04 stock insuficiente (409),
TC-07 seguridad del gateway (401/403), TC-08 comision 12 % y liquidacion al vendedor,
RN-05/RN-06/RN-07/RN-08, partida doble y regimen fiscal NIC.

```bash
npm test        # 45 tests: money 8 · gateway 6 · orders 7 · liquidaciones 5 · finance 19
npm run demo    # flujo real sobre el stack levantado (scripts/smoke.mjs)
```

## Observabilidad

Prometheus scrapea `/metrics` de los 8 servicios (`infra/prometheus/prometheus.yml`) y Grafana
provisiona el dashboard `BodegaHub · Core Engine` (latencia p99, peticiones/s, errores 5xx,
backlog de RabbitMQ). Los logs son JSON estructurados con `x-request-id` correlacionado
(AsyncLocalStorage).

## Conmutacion a Supabase (produccion)

1. Cree el proyecto Supabase (Postgres + Auth). 2. Ejecute `01_esquemas.sql` en la consola SQL.
3. Ejecute `99_rls.sql` (activa RLS por rol). 4. Iguale el secreto JWT HS256 y las URL de los
servicios al proyecto. La diferencia de implementacion queda aislada en `jwt.utils.ts`.