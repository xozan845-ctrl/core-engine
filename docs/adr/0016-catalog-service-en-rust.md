# ADR-0016: Reescritura de `catalog-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación en curso
- **Ámbito**: `catalog-service` (HTTP público e interno, PostgreSQL/RLS,
  RabbitMQ, Outbox, capacidades y validación JSON Schema)
- **Referencias**: [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0004](./0004-scheduling-service-en-rust.md),
  [ADR-0006](./0006-notifications-service-en-rust.md),
  [ADR-0010](./0010-logistics-service-en-rust.md),
  [ADR-0011](./0011-stores-service-en-rust.md)

## Contexto

`catalog-service` es el servicio NestJS del catálogo maestro, inventario y
precios (~9.600 líneas de TypeScript no-spec). Es la autoridad de stock del
lado del servicio y la pieza que consume `order.created` para reservar o
rechazar existencias (RN-03) y `devolucion.solicitada` para reintegrarlas
(RN-06), publicando los eventos de stock por Outbox.

Superficie principal:

- **Items unificados** (`/api/v2/catalog/items`): alta de producto, servicio y
  alquiler; atributos validados contra JSON Schema; variantes, imágenes,
  componentes/BOM, tarifas, series/serialización, listas de precio, precios por
  lista y escalas.
- **Inventario**: proveedores, lotes y consumo FEFO, ubicaciones, stock por
  ubicación, transferencias y movimientos; configuración de inventario.
- **Operaciones**: importación masiva, escaneo por código de barras, combos,
  promociones y ajustes de stock.
- **Legacy**: `productos` y `servicios` conservan la API y las tablas que aún
  consumen otros servicios; `productos` concentra la reserva atómica de stock,
  el histórico de precios y las consultas de stock usado por el consumidor y
  las operaciones.
- **Internos** (`/internal/items/**`, `/internal/inventario/**`): lote de items,
  item/precio/cotización por id, precios en lote, resumen y líneas de
  inventario.

Dependencias:

- PostgreSQL esquema `catalog` (items, variantes, imágenes, componentes,
  tarifas, series, listas/precios/escalas, lotes, proveedores, ubicaciones,
  stock por ubicación, movimientos, combos, promociones, configuración,
  históricos, reservas, eventos procesados y outbox).
- RabbitMQ: cola `catalog.pedidos` (`order.created`, `devolucion.solicitada`)
  con idempotencia por `event_id` y DLQ.
- `identity-service` y `storage-service` en las rutas legacy de productos
  (validación de empresa y de imágenes/medios).
- Capacidades `catalogo_items`, `inventario_stock`, `inventario_lotes` y
  `codigo_barras`; roles configurables con `catalogo.leer`/`catalogo.escribir`.
- Envelope `{data, meta}`, errores `{codigo, mensaje, detalles?}`, métricas
  Prometheus y logs JSON.

El servicio está **sin migración en curso**: no existe rama, worktree, ADR ni
sesión de trabajo sobre `catalog-service` distinta de esta. Las migraciones
activas son `api-gateway` (ADR-0015) y `orders-service` (ADR-0013). Se elige
`catalog-service` por su aislamiento de dominio (sin criptografía ni SDKs
externos), su contrato dorado y porque reutiliza el patrón PG + RLS + RabbitMQ
+ Outbox + capacidades ya validado en ADR-0006/ADR-0010/ADR-0011.

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismas rutas, métodos y códigos (201 en POST, 200 en GET/PATCH/PUT/DELETE, 400/401/403/404/409/503); envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores sin `meta` |
| Superficie interna | mismas rutas `/internal/items/**` y `/internal/inventario/**`, sin envelope, protegidas por `x-internal-key` |
| Datos | mismas tablas, columnas, `SELECT *`/`RETURNING`, jsonb como objeto/array y tipos de `node-postgres` (uuid/texto string, int4 número, int8 string, timestamptz ISO 8601 UTC con milisegundos y `Z`, date a medianoche local) |
| Multi-tenancy | `empresa_id`/`tenant_id` en todas las consultas; claims `sub`/`rol`/`tenant_id` en `request.jwt.claims` dentro de la transacción (R-DB-2) |
| Autorización | mismos roles por clase/método; `super_admin`/`admin` omiten capacidades; rol configurable solo con `@Permisos`; capacidad faltante ⇒ `403 CAPACIDAD_NO_ACTIVA` con `{empresa_id, capacidades}`; identity caído ⇒ `503 SERVICIO_NO_DISPONIBLE` |
| Validación | mismos DTO y shape `VALIDACION` con `detalles[{campo, restricciones}]`; reglas de atributos y extensiones con JSON Schema (`allErrors`, `strict:false`) y mismos mensajes de error |
| Items | tipos `producto`/`servicio`/`alquiler`, SKU único por empresa, estados `activo`/`inactivo`, baja lógica, atributos por esquema, variantes con SKU propio, imágenes ordenadas y componente/BOM con ciclos rechazados |
| Precios | montos enteros en centavos; precio de venta como `"1150.00"`; listas por empresa, precio por item/lista único, escalas por cantidad sin solapes, moneda ISO |
| Tarifas | vigencia por fechas, tarifa única activa por item, cotización con la tarifa aplicable y redondeo del contrato original |
| Series | unicidad por `serial`, estados y asignación/liberación ligadas a orden; devolución reintegra series en la misma transacción |
| Inventario | stock atómico por item/empresa; lotes con FEFO; stock por ubicación y transferencias atómicas; movimientos append-only con tipo y referencia; ajustes publican `stock.updated` |
| Stock (RN-03/RN-06) | reserva total o rechazo total por líneas no digitales; descuento y Outbox en una sola transacción; devolución reintegra stock, series y emite `stock.reintegrado` + `stock.updated`; cotización y entrega digital no comprometen inventario |
| Idempotencia | `catalog.eventos_procesados` por `event_id`; una sola reserva/devolución por evento; reintentos seguros |
| Eventos | mismos nombres y payloads (`stock.reservado`, `stock.fallido`, `stock.reintegrado`, `stock.updated`); Outbox `catalog.outbox` con poller 1 s, lote 50 y `pendiente→publicado` |
| RabbitMQ | exchanges `core-engine.events`/`core-engine.dlq`; cola `catalog.pedidos` con `order.created` y `devolucion.solicitada`; `x-intentos` máx. 3 y DLQ con republicación cada 10 s |
| Legado | `productos`/`servicios` conservan sus tablas, consultas y respuestas mientras existan consumidores; no se elimina ninguna ruta |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus con `core_engine_http_*` y etiquetas `{servicio, metodo, ruta, estado}`; logs JSON con `request_id`, `servicio` y nivel; `/docs` y `/docs-json` con el OpenAPI versionado |
| Runtime | puerto 3002 (`PORT`), variables `DATABASE_URL`, `RABBITMQ_URL`, `INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `STORAGE_SERVICE_URL`, `OUTBOX_TABLA`; fail-fast sin `DATABASE_URL`/`RABBITMQ_URL`; cierre ordenado |

### 2. Implementación destino

- Crate en la raíz de `packages/catalog-service/`, binario `catalog-service`;
  el TypeScript y el workspace npm se retiran en la conmutación.
- `axum` + `tokio`, `sqlx` (Postgres runtime Tokio), `lapin`, `reqwest`,
  `serde`/`serde_json`, `uuid`, `chrono`, `thiserror` y un validador JSON
  Schema acotado con paridad para los esquemas usados por el contrato.
- Capas: `http/` (router, envelope, rutas públicas e internas), `items/`,
  `precios/`, `tarifas/`, `series/`, `componentes/`, `inventario/`,
  `operaciones/`, `productos/` (stock y legado), `consumidor.rs`, `outbox.rs`,
  `rabbit.rs`, `auth.rs`, `db.rs`, `config.rs`.
- Inyección explícita por `AppState` con `Arc` (enmienda R-MS-3), igual que
  ADR-0004/0006/0010/0011.

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` y
   `cargo test` verdes.
2. Pruebas PostgreSQL: items/variantes/imágenes/atributos, listas/precios/
   escalas, tarifas/cotización, series, componentes, inventario/lotes/
   ubicaciones/transferencias, importación/escaneo/combos/promociones y
   movimientos.
3. Pruebas del consumidor: reserva atómica, rechazo total, cotización/digital,
   devolución con series y stock, idempotencia por `event_id` y Outbox.
4. Pruebas HTTP: envelope, guards, validación, 404/409, rutas internas y
   OpenAPI servido con `x-servicio`.
5. Cobertura Rust en el job `rust` de CI (`cargo-llvm-cov`) con gate medido y
   redondeado a la baja con margen de 1 punto.
6. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR y el índice | Decisión registrada |
| 2 | Andamiaje Rust: configuración, HTTP, envelope, errores, métricas, auth, DB/RLS, Outbox y RabbitMQ | `cargo test` verde |
| 3 | Items unificados: CRUD, atributos, variantes, imágenes, componentes y tarifas | Integración PostgreSQL verde |
| 4 | Precios, escalas, series, inventario, lotes, ubicaciones y transferencias | Integración PostgreSQL verde |
| 5 | Operaciones (importar, escanear, combos, promociones, movimientos), stock legacy y consumidor | Reserva/devolución idempotentes y Outbox verificado |
| 6 | Pruebas HTTP, Dockerfile, job `rust`, cobertura y README | Build y gates verdes |
| 7 | Conmutación: retirar TypeScript, scripts, compose, CI y docs | `npm run build` y smoke verdes |

## Consecuencias

### Positivas

- Un servicio NestJS menos y menor consumo en reposo.
- Stock, precios e inventario con tipos fuertes y pruebas de concurrencia.
- Reutiliza el patrón y los gates de ADR-0004/0006/0010/0011.

### Costes y riesgos

- Es el servicio más grande del repo: el puerto debe dividirse en PRs por
  subdominio para que cada uno sea revisable.
- La compatibilidad de `productos`/`servicios` con consumidores existentes
  exige pruebas de contrato además de las de dominio.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y las
  integraciones PostgreSQL en verde.
- Reserva/devolución de stock idempotentes, atómicas y con Outbox.
- OpenAPI, salud, métricas y rutas internas conservan el contrato.
- Smoke de `staging` verde en la primera promoción.

## Estado de implementación

En curso. El detalle de commits y verificaciones se registra en el PR final.
