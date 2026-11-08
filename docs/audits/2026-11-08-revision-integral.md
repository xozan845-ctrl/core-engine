# Revisión integral — Core Engine (2026-11-08)

> Auditoría independiente sobre `feat/pack-builder` (`bb602b5`), en un worktree
> limpio. Cubre lógica de negocio, arquitectura, tests, scripts y documentación.
> La fotografía de cumplimiento de testing vive en
> [`docs/rules/_meta/AUDIT.md`](../rules/_meta/AUDIT.md) (14ª auditoría).

## 1. Veredicto en una frase

Core Engine es una plataforma de comercio **multi-vertical** seria y bien
estructurada, con un núcleo de tests que sí aporta valor real, pero **HEAD está
en rojo** (cobertura de `identity-service` y contrato OpenAPI) y hay brechas
críticas de autorización, aislamiento multi-tenant y transaccionalidad que la
documentación no reflejaba.

## 2. Qué es el proyecto

Monorepo npm workspaces con **gateway + 11 microservicios NestJS** sobre
PostgreSQL 16 (compatible Supabase), RabbitMQ (topic + DLQ) y MinIO. Los montos
son enteros en centavos; el contrato es OpenAPI por servicio.

| Servicio | Puerto | Responsabilidad real |
|---|---|---|
| `api-gateway` | 8080 | Único punto de entrada: JWT HS256, políticas por ruta/rol, proxy, CORS, rate limit, readiness agregada. |
| `identity-service` | 3001 | Auth/sesiones, usuarios, empresas/miembros/invitaciones, configuración de sitio, capacidades, packs/verticales, extensiones y partes. |
| `catalog-service` | 3002 | Productos/variantes, servicios, inventario/lotes, combos, promociones, traslados; consumidor que reserva stock. |
| `stores-service` | 3003 | Tiendas, storefront público, ofertas con margen; consumidor que sincroniza stock visible. |
| `orders-service` | 3004 | Agregado Order por eventos, carrito, proyección `orden_vista`, timeline y transiciones. |
| `logistics-service` | 3005 | Envíos por orden/empresa, asignación y avance de estados. |
| `scheduling-service` | 3011 | Recursos, disponibilidad, bloqueos y citas; consume `order.created` para reservas onsite. |
| `commissions-service` | 3006 | Comisión por línea, espejo de pagos y liquidaciones quincenales (solo stage). |
| `finance-service` | 3007 | Partida doble, facturación, tributación, KPIs y workspace de comercio. |
| `field-service` | 3008 | Operación de campo por tenant: personal, rutas, visitas, asistencia, tracking. |
| `market-intelligence-service` | 3009 | Analítica/PostGIS: mapas de calor, rendimiento, demanda, forecast (solo dev). |
| `storage-service` | 3010 | Buckets MinIO por tenant, URLs prefirmadas, cuotas y media pública. |

### Cómo fluye una orden

1. `POST /api/v1/orders` valida precios contra stores y stock contra catalog, y
   escribe el evento `order.created` + outbox en una transacción
   (`packages/orders-service/src/pedidos/handlers/create-order-command.handler.ts`).
2. Catalog consume `order.created`, descuenta stock con `FOR UPDATE` y emite
   `stock.reservado`/`stock.fallido` (`packages/catalog-service/src/eventos/pedidos.consumer.ts`).
3. Orders marca `pagada`, emite `payment.procesado`; logistics crea el envío,
   commissions acredita por línea y finance asienta en partida doble.
4. El outbox (`packages/shared/src/outbox/outbox.service.ts`) publica al broker
   con confirm channel; los fallos van a DLQ con backoff.

## 3. ¿Los tests aportan algo bueno?

**Sí, el núcleo aporta.** La matriz real es **1.322 tests en 132 archivos y 13
workspaces**. Los de mayor valor:

- Dinero y estados: `packages/shared/src/money.spec.ts`,
  `money.property.spec.ts`, `order-state.spec.ts` (centavos enteros, márgenes,
  transiciones inválidas).
- Autorización de borde: `packages/api-gateway/src/proxy/gateway-auth.spec.ts`
  (63 casos de matriz de roles, deny-by-default, refresh token como access).
- Dinero/stock/outbox de orders: `create-order-command.handler.spec.ts` y
  `pedidos.service.spec.ts`.
- Stock atómico y precios: `catalog-service/.../productos.service.spec.ts`,
  `stores-service/.../ofertas.service.spec.ts`.
- Auth/sesiones/tenant: `identity-service/.../auth.service.spec.ts`,
  `empresas.service.spec.ts`, `configuracion-sitio.service.spec.ts`.
- Storage (cuotas, tenant, Range/ETag) y logística (guía idempotente,
  aislamiento por flota).
- Integración real API+BD: `scripts/integration/*.test.mjs` (stock, configuración
  de sitio, scheduling con exclusión GiST, packs, migración de tenant).
- Property/mutation reales en `shared`; motor de gobernanza con un fixture
  centinela que prueba que un fallo produce exit code 1.

**Pero el nivel global está sobrevendido.** Hay una capa grande de tests de
delegación que inflan el conteo sin validar requisitos:

- `packages/scheduling-service/src/scheduling/scheduling.service.spec.ts:201`
  termina en `expect(true).toBe(true)`.
- `packages/finance-service/src/comercio/comercio.controller.spec.ts:12` invoca
  >50 métodos con un `Proxy` y solo exige `resolves.toBeDefined()`; además
  asserta `metodos.length > 50`.
- `packages/shared/src/order-state.property.spec.ts:29` es una propiedad vacua:
  si la transición es válida, el destino está en el mismo conjunto con el que se
  define la validez.
- `packages/shared/src/constants.spec.ts` y
  `packages/shared/src/events/contracts.spec.ts` congelan literales; solo aporta
  la unicidad de nombres.
- Varios caminos positivos de authz en `stores-service` e `identity-service`
  solo comprueban `resolves.toBeDefined()`.
- Casi todos los unit tests mockean `PgService` y asertan fragmentos SQL: el SQL
  real, el event store y el outbox no se ejecutan en unitarios.

**Huecos que importan más que el porcentaje:**

1. `shared/src/outbox/outbox.service.ts` y `shared/src/db/pg.service.ts` **no
   tienen spec** (es la deuda que queda en `spec-baseline.json`).
2. El handler de orders usa `EventStoreFake`: append/versionado/concurrencia
   optimista reales no se prueban.
3. La saga completa (order→stock→payment→commission→accounting) no tiene prueba
   de integración; los consumers se prueban con Rabbit/PG falsos.
4. `identity-service` bajó a **80,42/71,31/74,38** (umbral 89/78/85) por los
   módulos nuevos packs/partes/extensiones/capacidades; tres controllers no
   tienen spec.
5. `market-intelligence` (45,57/35,22) y `shared` (67,85/63,48) siguen lejos del
   objetivo 80/70.

## 4. ¿Los scripts aportan algo bueno?

El **motor de gobernanza** (`scripts/engine` + `scripts/rules` + `scripts/tools`)
es la mejor pieza de ingeniería del repo: suites declarativas, manifiesto,
descubrimiento aislado, centinela de falso verde y autotests. `npm run
rules:check` pasa con 59 tests y 41/238 IDs de regla con check ejecutable.

Lo que **no** aportaba o estaba roto:

- **`qa-harness` no arrancaba**: todos los scripts apuntaban a `src/index.ts`
  cuando el archivo real es `src/index.js`; además importaba `bcryptjs` sin
  declararlo y podía dar **falso verde** (una suite con `pass:false` y cero
  resultados sumaba cero fallos). Corregido en este worktree.
- **`--format`, `--base-url`, `--load`, `--concurrency`, `--fuzz`,
  `--no-cleanup`** se parseaban pero no se aplicaban. Corregido.
- **`getQueueDepth`/`getDLQDepth`** leían `info.message_count` en vez de
  `info.body.message_count`: siempre 0. Corregido.
- `promotion:check` y `multi-vertical:check` **no están en ningún workflow**;
  son manuales. `coverage:baseline`, `version:bump` y `start:field` también.
- `qa-harness/15-static` duplica `npm test`, `lint` y `build`; es lento y no
  está en CI.
- El README de `qa-harness` describía una implementación TypeScript de 15
  suites que ya no existe (hoy son 22 suites en JavaScript). Corregido.

## 5. Hallazgos de lógica y seguridad

### Críticos

1. **Dos rutas de autenticación responden 403 por el gateway.**
   `POST /api/v1/auth/cambiar-contrasena` y `POST /api/v1/auth/vincular-personal`
   existen (`packages/identity-service/src/auth/auth.controller.ts:229,279`) y
   están en OpenAPI, pero no tienen política en `POLITICAS_RUTAS`; la regla
   deny-by-default devuelve 403
   (`packages/api-gateway/src/proxy/gateway-auth.middleware.ts:295-299,342`).
2. **Fuga de datos en market-intelligence.** `/inteligencia/me/mapa-calor` y
   `/me/rendimiento` leen `x-user-personal`
   (`packages/market-intelligence-service/src/inteligencia/controllers/metricas.controller.ts:30-39,51-60`).
   Un vendedor normal no tiene ese claim; `vendedor_id` queda `undefined` y el
   repositorio devuelve **todos** los vendedores
   (`.../repositories/inteligencia.repository.ts:30-37,59-69`). Además
   `POST /inteligencia/ventas/registrar` acepta `@Body() data: any` sin
   idempotencia y el gateway lo abre a comprador/logística.

### Altos

3. **Secretos con default inseguro**: `JWT_SECRET ?? 'dev_secret'`
   (`gateway-auth.middleware.ts:310`, `identity-service/src/auth/auth.service.ts:56`)
   y `ADMIN_PASSWORD ?? 'AdminCore Engine2026!'`
   (`identity-service/src/seed/seed.service.ts:19`).
4. **Outbox no es exactly-once**: el `SELECT ... FOR UPDATE SKIP LOCKED` corre en
   autocommit y las filas quedan libres antes de publicar; ticks solapados o
   réplicas pueden duplicar eventos. El filtro `creado_en > NOW() - INTERVAL '30
   minutes'` abandona eventos antiguos y el `.catch(() => undefined)` oculta
   fallos (`packages/shared/src/outbox/outbox.service.ts:29-39,78-104`).
5. **Event store sin concurrencia optimista real**: `MAX(version)+1`
   (`packages/orders-service/src/pedidos/repositories/order-event-store.ts:31,48`)
   y el índice `(aggregate_id, version)` **no es único**
   (`infra/db/init/01d_es_orders.sql:15`).
6. **Pasos de saga no atómicos**: catalog confirma `reservarStock` y **después**
   inserta en outbox; si el outbox falla, la reserva queda hecha y el reintento
   choca con la PK de `catalog.reservas_ordenes` y termina en DLQ
   (`packages/catalog-service/src/eventos/pedidos.consumer.ts:72-99`,
   `productos.service.ts:881-960`). En orders, `pagada` se confirma antes de
   emitir `payment.procesado`. En commissions, un fallo de outbox puede dejar la
   línea sin evento.
7. **RLS inconsistente**: `99_rls.sql` protege `orders.proyeccion_ordenes`
   (`infra/db/supabase/99_rls.sql:282-290`) pero el código usa
   `orders.orden_vista` (`infra/db/init/01d_es_orders.sql:20`). Varios
   consumidores (logistics, stores, finance) no abren contexto de tenant.
8. **Comisión 12 % hardcodeada en tres sitios** (`orders-service` handler:170,
   `pedidos.service.ts:15`, trigger SQL `01d:87`) mientras commissions usa
   `COMMISSION_RATE`. Si la tasa cambia, la vista de orden y el ledger no cuadran.
9. **Storage**: la cuota se verifica fuera de la transacción y `sumarUso` no
   revalida (`packages/storage-service/src/objetos/objetos.service.ts:167,255-267`);
   `eliminar` borra en MinIO antes del commit de BD (`:371-393`).
10. **Scheduling**: el chequeo de bloqueo y la inserción de cita no comparten una
    constraint con `bloqueos`; un bloqueo concurrente puede dejar una cita
    solapada (`scheduling.service.ts:149-166`). Solo se traduce `23P01`, no
    `23505` (`:345-349`).

### Medios

11. Acoplamiento a esquemas ajenos: orders lee `identity.empresa_capacidades`,
    scheduling/field leen `identity.partes`, identity escribe `stores.tiendas`.
12. Capacidades parcialmente aplicadas: solo catalog, stores, scheduling e
    identity montan `CapabilityGuard`; orders/logistics/finance/commissions/
    field/storage no.
13. RabbitMQ: `JSON.parse` fuera del `try` (`shared/src/rabbitmq/rabbit.service.ts:113`),
    así un mensaje corrupto no se ackea ni va a DLQ.
14. Código/colas muertas: `reproyectar` es un no-op, `OrderEventStore.append`,
    `actualizarEstadoActualizado` y `OutboxService.reenviar` no se usan; varias
    colas no tienen consumidor.

## 6. Estado de gates en HEAD

| Gate | Estado | Detalle |
|---|---|---|
| G-1 | ❌ | `npm test` aborta en `identity-service` por cobertura. |
| G-3 | ❌ | 80,42/71,31/74,38 vs 89/78/85. |
| G-5 / R-C-7 | ❌ | `contract:breaking` sale 1 por `parte_id` obligatorio en 4 respuestas. |
| G-7 | ✅ | Lint 0 errores/45 warnings; build 0. |
| G-8 | ✅ | `swagger:export` sin diff; 334 operaciones (310 públicas + 24 internas). |
| G-4 / G-6 | ⏸ | 14 tests de integración; E2E solo manual/release. |

## 7. Documentación alineada en este worktree

- `README.md`: 11 microservicios, `scheduling-service:3011`, 13 workspaces,
  1.322 tests, 334 operaciones OpenAPI, readiness 9/10, variables de entorno.
- `docs/rules/test/06`, `07` y `README`: 13 workspaces, baseline de 2 specs,
  comando `npm test` correcto, estado real de G-1/G-3.
- `docs/rules/_meta/AUDIT.md` (14ª auditoría) y `AUDIT-HISTORY.md`.
- `.github/README.md`, `docs/runbook-entrega.md`, `docs/dockploy-setup.md`,
  `docs/decisiones.md`, `docs/rules/db/02-supabase.md`.
- `.env.stage.example` y `.env.production.example`: MinIO público 443/HTTPS.
- `qa-harness/README.md` y `qa-harness/package.json`.

## 8. Recomendaciones priorizadas

1. **Cerrar el rojo de HEAD**: specs de `capacidades`, `extensiones` y `partes`
   en identity; decidir si `parte_id` es obligatorio y registrar la excepción de
   contrato en el mismo PR.
2. **Arreglar autorización**: añadir políticas para `cambiar-contrasena` y
   `vincular-personal`; acotar `/inteligencia/me/*` al vendedor autenticado y
   validar el body de `ventas/registrar` con DTO.
3. **Quitar defaults de secretos** en producción (fallar al arrancar si faltan).
4. **Hacer el outbox exactly-once**: reclamar filas en una transacción con
   `FOR UPDATE SKIP LOCKED` + marca `publicando`, sin ventana de 30 min, y
   propagar errores al logger/métrica.
5. **Añadir índice único `(aggregate_id, version)`** y reintentos optimistas en
   el event store.
6. **Unificar la tasa de comisión** en una sola fuente y probar el desglose
   línea a línea.
7. **Cubrir outbox/pg/event store con specs** y añadir una prueba de integración
   de la saga completa.
8. **Subir thresholds** de identity, shared y market por ratchet, y sustituir
   los tests de delegación por asserts de estado/montos.

## 9. Evidencia ejecutada

```text
npm ci                                      → 0
npm run lint                                → 0 (45 warnings)
npm run build                               → 0
npm run swagger:export                      → 0 (sin diff; 310 + 24 ops)
npm test                                    → 1 (identity-service, cobertura)
npm run contract:breaking -- --base-ref origin/main → 1 (4 parte_id)
npm run rules:check                         → 0 (59 tests, 11 suites)
npm run test:engine                         → 0 (24 tests)
npm run test:smoke                          → 0 (62 tests)
npm audit --audit-level=high                → 0
npm audit --audit-level=moderate            → 12 (tooling)
```
