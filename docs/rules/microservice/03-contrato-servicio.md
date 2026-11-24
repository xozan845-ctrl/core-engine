# 03 — Contrato de servicio (manifiesto y perfiles)

Aplica a: todos los servicios desplegables de `packages/*`. Define el **contrato
declarativo** que convierte la arquitectura en algo verificable: cada servicio
publica `packages/<servicio>/service.json`, y la suite `services-contract`
contrasta ese manifiesto con el crate, el Dockerfile, los composes, el esquema,
los módulos, el arranque, la observabilidad y los eventos reales.

> **Fuente única de verdad (R-AF-3):** `SERVICES`, puertos, perfiles, umbrales de
> cobertura y la matriz de CI se derivan de estos manifiestos. Ningún script
> vuelve a hardcodear la lista de servicios.

## Reglas

| ID | Regla |
|---|---|
| R-MS-9 | **Manifiesto de servicio obligatorio y ejecutable**: cada `packages/<servicio>/service.json` declara `metadata` (name, boundedContext, description, owner, lifecycle, tags) y `spec` (profile, runtime, port, schema, outboxTable, capabilities, publicApi, internalApi, environments, dependencies.sync, events.publishes/consumes, slo, docs) conforme a `scripts/config/service-schema.json`. El manifiesto se actualiza en el **mismo PR** que el cambio de contrato, puerto, esquema, capacidad, evento, dependencia o SLO. Un servicio con `Dockerfile` sin manifiesto no entra al catálogo y la suite falla. |
| R-MS-10 | **Perfiles arquitectónicos y módulos exigidos**: todo servicio declara uno de cinco perfiles y cumple sus módulos mínimos. `edge` (gateway): `auth/config/errors/state/logs/metrics/http/policy/proxy/rate_limit/readiness`, sin BD. `core`: `auth/config/db/errors/state/logs/metrics/http`, sin broker. `transactional`: como `core` + `rabbit` + `outbox` (publica eventos). `consumer`: como `core` + `rabbit` (consume, sin outbox propio). `olap`: como `consumer` con proyecciones de lectura. Todo módulo exigido se declara en `src/lib.rs`; el perfil no es una etiqueta decorativa: la suite lo verifica. |
| R-MS-11 | **Salud y readiness separadas**: todo servicio expone `GET /health` (liveness, sin dependencias externas) y `GET /metrics`. Todo servicio con base de datos expone además `GET /ready`, que devuelve `503` cuando PostgreSQL o RabbitMQ no están disponibles y `200` cuando el servicio puede atender tráfico. `health` nunca se usa como readiness ni al revés. La deuda existente de `/ready` está congelada en `scripts/config/architecture-baseline.json` y sólo puede reducirse. |
| R-MS-12 | **Puerto único y proceso 12-factor**: cada servicio escucha en `0.0.0.0:<PORT>` (un solo puerto de aplicación), declara el mismo valor en `EXPOSE` del Dockerfile, `PORT` del compose y `spec.port` del manifiesto. Es un proceso **sin estado local**: la configuración llega por entorno, los logs van a stdout/stderr y el estado vive en PostgreSQL, RabbitMQ u object storage. Prohibido escribir estado de negocio en el filesystem del contenedor. |
| R-MS-13 | **Catálogo con ownership y ciclo de vida**: `metadata.owner` identifica al equipo responsable (nunca una persona), `metadata.lifecycle` es `experimental`, `production` o `deprecated`, y `spec.slo` declara disponibilidad y latencia p95 alineadas con `docs/slo.md`. Un cambio de owner, lifecycle o SLO se revisa con el mismo PR que el código. El catálogo generado (`docs/architecture/catalogo-servicios.md`) es la vista humana de estos datos. |
| R-MS-14 | **Contrato de eventos con un solo dueño**: los eventos se declaran en `spec.events.publishes/consumes`; el nombre que aparece en el manifiesto debe existir literalmente en `src/`. Todo evento publicado tiene **un único servicio dueño**; todo evento consumido tiene un publisher local declarado o figura en `EXTERNAL_EVENTS` si lo produce un sistema externo. Los publicadores usan outbox transaccional (R-AR-3); los consumidores son idempotentes por `event_id` (R-DI-4) y enrutan a DLQ tras agotar reintentos (R-AR-4). Renombrar o retirar un evento exige migración de consumidores y ADR. |
| R-MS-15 | **Contrato de observabilidad**: todo servicio expone `/metrics` con las métricas RED (`core_engine_http_requests_total`, `core_engine_http_request_duration_seconds`, errores 5xx y 4xx relevantes) y logs JSON con `x-request-id`; el identificador se propaga por HTTP y por el outbox hacia RabbitMQ. Un servicio sin estas señales no es operable y no entra a producción (R-TM-1/R-TM-4/R-OB-1/R-OB-2). |
| R-MS-16 | **Propiedad exclusiva de los datos**: ningún servicio ejecuta SQL contra el esquema de otro (`FROM/JOIN/INTO/UPDATE/DELETE` sobre un `spec.schema` ajeno). La integración es por API interna, evento o proyección local en el esquema propio (R-DB-1). La suite detecta el acoplamiento y la deuda existente está congelada en `architecture-baseline.json`; no se admite deuda nueva. |

## Manifiesto mínimo

```json
{
  "$schema": "../../scripts/config/service-schema.json",
  "apiVersion": "core-engine.io/v1",
  "kind": "Service",
  "metadata": {
    "name": "orders-service",
    "boundedContext": "orders",
    "description": "CQRS + Event Sourcing del ciclo del pedido.",
    "owner": "core-platform",
    "lifecycle": "production",
    "tags": ["rust", "transactional", "cqrs"]
  },
  "spec": {
    "profile": "transactional",
    "runtime": "rust",
    "port": 3004,
    "schema": "orders",
    "outboxTable": "orders.outbox",
    "capabilities": ["orders", "fidelizacion"],
    "publicApi": true,
    "internalApi": true,
    "environments": ["dev", "stage", "prod"],
    "dependencies": { "sync": ["inventory-service"] },
    "events": {
      "publishes": ["order.created", "payment.procesado"],
      "consumes": ["stock.reservado", "stock.fallido"]
    },
    "slo": { "availability": 99.5, "latencyP95Ms": 1500 },
    "docs": { "readme": "README.md", "openapi": "docs/openapi/orders-service" }
  },
  "quality": { "coverage": { "lines": 62, "functions": 63 } }
}
```

## Verificación

- Suite `services-contract` (`scripts/rules/microservices/service-contract.test.mjs`).
- Esquema `scripts/config/service-schema.json`.
- Deuda congelada `scripts/config/architecture-baseline.json` + `npm run baseline:guard`.
- Catálogo y matriz de eventos generados con `npm run architecture:report`; CI falla si hay diff (`npm run architecture:check`).
