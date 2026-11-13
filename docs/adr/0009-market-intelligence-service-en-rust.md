# ADR-0009: Reescritura de `market-intelligence-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación completa; pendiente el smoke de `staging` en la primera promoción de la imagen
- **Ámbito**: `market-intelligence-service` (HTTP público, PostgreSQL/PostGIS, RabbitMQ, tareas programadas, métricas)
- **Referencias**: [ADR-0004](./0004-scheduling-service-en-rust.md), [ADR-0006](./0006-notifications-service-en-rust.md), [ADR-0008](./0008-logistics-service-en-rust.md)

## Contexto

`market-intelligence-service` era un servicio NestJS (~1.300 líneas) que:

- Expone 14 rutas bajo `/api/v2/inteligencia/**`: mapa de calor, rendimiento,
  cobertura, demanda, tendencias, resumen, ingesta manual, forecast, anomalías,
  score de vendedor y calidad del pipeline.
- Exige la capacidad `inteligencia_mercado` vía identity; no aplica guardas de
  rol locales (el gateway restringe `/api/v2/inteligencia/*` y `/me/*`).
- Consume tres colas: `intelligence.venta_geo` (`venta.geolocalizada`),
  `intelligence.order_completado` (`comision.acreditada`) e
  `intelligence.stock_updated` (`stock.updated`), con idempotencia por
  `event_id`.
- Agrega hechos, puntos de calor PostGIS, rendimiento, cobertura, métricas de
  producto y tendencias en una transacción.
- Ejecuta dos crons: features estadísticas cada 5 minutos y refresco
  concurrente de vistas materializadas cada 30 segundos.
- Valida el Data Contract de ventas con JSON Schema y audita los inválidos.

Se reescribe en Rust **sin cambiar contratos, modelos ni lógica**, siguiendo el
patrón de ADR-0004/0006/0008.

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismas 14 rutas y 200/201; envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores `{codigo, mensaje, detalles?}` sin `meta` |
| Autorización | capacidad `inteligencia_mercado` consultada a identity; `super_admin`/`admin` pasan; resto exige `x-tenant`; `/me/*` exige `x-user-personal` (401) y la ingesta de un `vendedor` exige ficha de personal (403) |
| Query | `FiltrosInteligenciaDto` valida `desde`/`hasta` ISO 8601, `sku` string y `vendedor_id` UUID v4 y descarta el resto; las rutas con tipo intersección/`any` no validan y conservan el comportamiento de `parseInt`/`parseFloat` |
| Defaults | `dias` 7 y rango 1..90; `limite` 50/100 y topes 200/720; `parseInt(x) \\|\\| default` |
| Datos | mismas tablas, columnas, SQL y upserts (`ON CONFLICT`), incluidos los buckets de mercado (ADR-27) |
| Tipos JSON | replicar `node-postgres`: `bigint` y `numeric` como string, `int4` como número, `double precision` como número, `date`/`timestamptz` como ISO 8601 UTC (R-C-4) |
| Idempotencia | `intelligence.eventos_procesados` por `event_id`; ingesta manual sin `evento_id` usa `md5(order_id)::uuid` |
| Data Contract | JSON Schema draft-07 (requeridos, tipos, rangos, enum de género, `additionalProperties: false`) + reglas de negocio (`monto_cents > 0`, `lat` en rango) |
| Calidad | score = campos opcionales completos / 10; High `> 0.9`, Medium `> 0.5`, resto Low; métricas `core_engine_intelligence_*` |
| Eventos | mismas 3 colas y routing keys; el evento inválido se audita y se marca procesado |
| Crons | features cada 5 min (5 consultas) y refresco concurrente de las 2 vistas cada 30 s; gauge `core_engine_intelligence_view_refresh_duration_seconds` |
| RabbitMQ | exchanges `core-engine.events`/`core-engine.dlq`, DLQ por cola, `x-intentos` máx. 3 y republicación cada 10 s |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus; logs JSON con `request_id`, `servicio` y nivel |
| Runtime | puerto 3009 (`PORT`), mismas variables, fail-fast sin `DATABASE_URL`/`RABBITMQ_URL`, cierre ordenado |

### 2. Implementación destino

- Crate en la raíz de `packages/market-intelligence-service/`, con binario
  `market-intelligence-service`; se retira el TypeScript en la conmutación.
- `axum` + `tokio` para HTTP, `sqlx` para PostgreSQL/PostGIS, `lapin` para
  RabbitMQ, `reqwest` para identity, `serde`/`serde_json`, `uuid`, `chrono`,
  `chrono-tz` y `thiserror`.
- Capas: `http/` (router, envelope, rutas), `inteligencia/` (consultas),
  `agregador.rs`, `calidad.rs`, `consumidor.rs`, `tareas.rs`, `outbox.rs`,
  `rabbit.rs`, `auth.rs`, `db.rs`, `config.rs`.
- Inyección explícita por estado compartido (`AppState` con `Arc`).

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y
   `cargo test` verdes.
2. Pruebas de integración PostgreSQL/PostGIS (`tests/inteligencia_pg.rs`):
   agregación idempotente, tipos JSON de `node-postgres`, comisión/stock,
   features, forecast, score, calidad y rechazo/auditoría de eventos inválidos.
3. Los OpenAPI versionados no cambian; el binario sirve el mismo documento.
4. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR y el índice | Decisión registrada |
| 2 | Andamiaje Rust: configuración, HTTP, métricas, envelope y errores | `cargo test` verde |
| 3 | Consultas analíticas y validación de query/DTO | Paridad de tipos JSON |
| 4 | Agregador, Data Contract y consumidor de 3 colas | Integración PostGIS verde |
| 5 | Crons de features/vistas, Outbox e imagen Docker | Gates verdes |
| 6 | Conmutación: retirar TypeScript y actualizar scripts raíz | `npm run demo` y OpenAPI verdes |
| 7 | Smoke de `staging` | Imagen Rust validada antes de producción |

## Consecuencias

### Positivas

- Un servicio NestJS menos y menos consumo en reposo.
- Tipado estricto en un dominio con SQL analítico y tipos `numeric`.
- Reutiliza el patrón y los gates de las migraciones anteriores.

### Costes y riesgos

- La paridad de tipos de `node-postgres` es sensible (`bigint`/`numeric` como
  string); queda cubierta por pruebas de integración.
- PostGIS es obligatorio para la agregación y el mapa de calor.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  integración PostgreSQL/PostGIS en verde.
- Mismas respuestas (incluidos tipos string de `bigint`/`numeric`) y mismos
  eventos/colas.
- OpenAPI, salud y métricas conservan el contrato.
- Smoke de `staging` verde en la primera promoción.

## Estado de implementación (2026-11-13)

- Crate completo en `packages/market-intelligence-service/` con 53 pruebas
  unitarias y 4 de integración PostgreSQL/PostGIS.
- TypeScript retirado; el servicio entra en `RUST_SERVICES` y su gate pasa al
  job `rust` de CI con cobertura 63/60 (medida: 64,41 % líneas / 61,82 %
  funciones).
- El job `rust` usa una imagen `postgis/postgis:16-3.4` para poder ejecutar la
  integración geoespacial.
- Imagen Docker multi-stage equivalente (puerto 3009) y compose de desarrollo
  apuntando al Dockerfile del servicio.
