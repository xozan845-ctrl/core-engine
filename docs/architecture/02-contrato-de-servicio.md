# 02 — Contrato de servicio

Un microservicio de Core Engine es **desplegable, observable, aislado y
evolucionable de forma independiente**. Este documento define el contrato que
debe cumplir; la versión ejecutable vive en `packages/<servicio>/service.json` +
`scripts/rules/microservices/service-contract.test.mjs`.

> **Regla de oro:** si una afirmación de este contrato no se puede comprobar en
> un PR, no es una regla: es una aspiración. Por eso cada fila tiene evidencia y
> una fitness function.

## 1. Definición de “servicio conforme”

| Dimensión | Requisito | Evidencia | Regla |
|---|---|---|---|
| Identidad | Carpeta, crate, binario, imagen y manifiesto tienen el mismo nombre | `service.json`, `Cargo.toml`, `Dockerfile` | R-MS-9 |
| Dominio | Un bounded context y un lenguaje ubicuo; no es un servicio “por entidad” | `metadata.boundedContext`, README | R-MS-9 |
| Propiedad | Owner de equipo y lifecycle declarados | `metadata.owner/lifecycle` | R-MS-13 |
| Puerto | Un puerto de aplicación, `0.0.0.0`, `EXPOSE`/`PORT`/manifiesto iguales | Dockerfile, compose, `service.json` | R-MS-12 |
| Perfil | `edge`/`core`/`transactional`/`consumer`/`olap` con módulos mínimos | estructura de `src/` | R-MS-10 |
| Datos | Esquema propio, migración `CREATE SCHEMA`, sin SQL a esquemas ajenos | `schema`, `infra/db/init` | R-DB-1/R-MS-16 |
| Eventos | Publica por outbox; consume idempotente; eventos declarados y con dueño | `events`, `outbox.rs`, `rabbit.rs` | R-AR-3/R-DI-4/R-MS-14 |
| Salud | `/health` liveness y `/ready` readiness si usa BD | `http/mod.rs` | R-MS-11 |
| Métricas | `/metrics` con RED y métricas de negocio | `metrics.rs` | R-TM-1/R-TM-4 |
| Logs | JSON estructurado, sin PII, con `request_id` | `logs.rs` | R-OB-1/R-OB-2/R-OB-5 |
| Config | Fail-fast al arrancar; env sólo en `config.rs`/`db.rs` | `config.rs`, `main.rs` | R-MS-5 |
| Ciclo de vida | `SIGINT`/`SIGTERM` + apagado ordenado | `main.rs` | R-MS-6 |
| Contrato | OpenAPI público/interno versionado y sincronizado | `docs/openapi/<servicio>` | R-DO-6/R-C-7 |
| Documentación | README con bounded context, capacidades, tablas y env | `README.md` | R-MS-8 |
| Calidad | fmt, clippy `-D warnings`, tests y cobertura por umbral | CI `rust` | R-QA-1/R-COV-1 |
| Seguridad | Sin secretos, authz por rol/capacidad, RLS, rutas internas protegidas | auth/RLS/CI | R-DS-1..7 |
| Operación | SLO, runbook y alertas asociadas | `service.json`, `docs/slo.md` | R-MS-13 |

## 2. Perfiles y mínimos

| Perfil | Servicios hoy | Base de datos | Broker | Outbox | Módulos extra obligatorios |
|---|---|---|---|---|---|
| `edge` | `api-gateway` | No | No | No | `policy`, `proxy`, `rate_limit`, `readiness` |
| `core` | `identity`, `item`, `pricing`, `field` | Sí | No | No | — |
| `transactional` | `inventory`, `orders`, `logistics`, `scheduling`, `commissions`, `finance`, `storage` | Sí | Sí | Sí (publica) | `rabbit`, `outbox` |
| `consumer` | `stores`, `notifications` | Sí | Sí | No | `rabbit`, consumidor idempotente |
| `olap` | `market-intelligence`, `analytics` | Sí | Sí | No | `rabbit`, proyecciones/materialized views |

Un servicio puede tener módulos adicionales (`lifecycle`, `consumer`,
`consumidor`, `pasarelas`, `pdf`, `storage`) según su dominio. El perfil fija el
**mínimo**, no el máximo.

### ¿Puede un servicio cambiar de perfil?

Sí, con ADR y migración explícita. Ejemplos:

- `core` → `transactional` cuando empieza a publicar eventos: debe añadir outbox,
  relay, métricas de outbox y consumidores de compensación.
- `transactional` → `consumer`: sólo si deja de ser dueño de un evento; hay que
  migrar consumidores y retirar el outbox en el mismo PR.
- Cualquier perfil → `olap`: sólo para analítica; no puede mutar el dominio
  transaccional.

## 3. Contrato HTTP

### 3.1 Superficies

| Superficie | Prefijo | Autenticación | Visible al cliente | Regla |
|---|---|---|---|---|
| Pública | `/api/v2/...` | JWT del gateway | Sí | R-GW-7 |
| Interna | `/internal/...` | `x-internal-key` | No (el gateway la bloquea) | R-DS-5 |
| Infraestructura | `/health`, `/ready`, `/metrics`, `/docs` | Según entorno | Sólo operación | R-MS-11/R-TM-1 |

### 3.2 Formato

- **Éxito:** `{ "data": <payload>, "meta": { "timestamp", "path", "method", "statusCode" } }`.
- **Error:** `{ "codigo": "UPPER_SNAKE_CASE", "mensaje": "...", "detalles"?: ... }`.
- **Montos:** centavos enteros; el contrato público documenta la representación
  decimal con dos posiciones (ADR-01).
- **Fechas:** ISO 8601 UTC con `Z`.
- **Paginación:** cursor/keyset para historiales; nunca `OFFSET` profundo (R-DB-7).
- **Idempotencia:** `Idempotency-Key` en `POST/PATCH/DELETE` críticos; replay de
  la respuesta original y detección de reutilización (ADR-0033).
- **Capacidades:** el servicio valida la capacidad del tenant antes de operar;
  `403 CAPACIDAD_NO_ACTIVA` si no está activa.

### 3.3 Versionado y evolución

- Cambio aditivo: mismo `/api/v2`.
- Cambio rompiente: nueva versión de superficie y ventana de convivencia.
- El contrato OpenAPI se regenera y se versiona en el mismo PR (R-DO-6).
- `oasdiff`/revisión de breaking changes es el siguiente paso recomendado para CI;
  hoy la protección es el diff del spec generado y los tests de contrato.

## 4. Contrato de eventos

### 4.1 Declaración

En `service.json`:

```json
"events": {
  "publishes": ["order.created", "payment.procesado"],
  "consumes": ["stock.reservado", "stock.fallido"]
}
```

La suite comprueba que:

1. el nombre existe literalmente en `src/`;
2. todo evento consumido tiene un publisher local o es externo declarado;
3. un evento tiene un solo servicio dueño;
4. si publica, existe `outbox.rs`; si consume, existe `rabbit.rs`.

### 4.2 Garantías

| Garantía | Implementación | Regla |
|---|---|---|
| Atomicidad negocio+evento | Outbox en la misma transacción | R-AR-3 |
| Entrega al menos una vez | Relay con confirmación + reintentos | R-AR-4 |
| Idempotencia | `eventos_procesados` por `event_id` | R-DI-4 |
| Orden por agregado | Una cola por consumidor y clave de agregado | R-AR-4 |
| Trazabilidad | `request_id` en outbox y headers | R-OB-2 |
| No bloqueo | DLQ tras agotar reintentos | R-AR-4 |
| Evolución | Cambios aditivos; rename/retiro con ADR | R-MS-14 |

### 4.3 Nombres

- Eventos de dominio en español cuando el dominio lo es (`stock.reservado`,
  `devolucion.solicitada`); algunos históricos en inglés (`order.created`,
  `payment.procesado`) se conservan por compatibilidad y no se renombran sin
  migración.
- Formato: `sustantivo.verbo_en_pasado` o `sustantivo.estado`.
- Un evento describe un **hecho**, no una orden. Si necesitas mandar una orden,
  es un comando y va a una cola específica, no al topic de dominio.

## 5. Datos

### 5.1 Propiedad

- Cada servicio posee **un esquema** (`spec.schema`) y ninguna tabla de otro.
- Las lecturas cross-schema están prohibidas; se resuelven con:
  - API interna de solo lectura, con timeout y degradación;
  - evento + proyección local en el esquema propio;
  - vista materializada **propia** alimentada por eventos.
- Las escrituras cross-schema están prohibidas sin excepción. La deuda actual se
  reduce, no se amplía.

### 5.2 Migraciones y RLS

- Migración aditiva en `infra/db/init/` + manifiesto de hash (R-DB-6).
- Toda tabla nueva: RLS habilitado + política en `infra/db/supabase/99_rls.sql`
  (R-SB-2).
- Índices para FKs lógicas y filtros frecuentes (R-DB-4).
- Soft delete en datos transaccionales (R-DB-5).

### 5.3 Tenancy

- `empresa_id` y `negocio_id` en toda escritura cuando aplique.
- El contexto se inyecta en la sesión de Postgres para RLS (`set_config`).
- Los tests de integración cubren aislamiento entre empresas/negocios.

## 6. Seguridad

| Control | Requisito | Regla |
|---|---|---|
| Autenticación | JWT HS256 validado en el gateway; claims mínimos | R-GW-3 |
| Autorización | Política por ruta/rol + capacidad de negocio | R-GW-3/R-DS-3 |
| Rutas internas | `x-internal-key` obligatorio | R-DS-5 |
| Secretos | Sólo entorno/gestor; nunca en repo ni imagen | R-DS-1/R-CD-7 |
| PII | Sin PII en logs; cifrado en reposo para secretos | R-DS-2/R-OB-5 |
| Entrada | Validación tipada + límites + mass assignment | R-DS-4 |
| Rate limit | En el gateway, no en los servicios | R-DS-6 |
| Dependencias | `cargo audit`/`npm audit` sin high/critical | R-DS-7/R-QA-6 |

## 7. Observabilidad y SLO

- `/health` liveness; `/ready` readiness con 503; `/metrics` Prometheus.
- RED + métricas de negocio relevantes (outbox pendiente, DLQ, pagos fallidos).
- Logs JSON con `servicio`, `nivel`, `request_id`, `ruta`; sin tokens ni PII.
- SLO por servicio en `spec.slo`, alineado con `docs/slo.md`; alertas con runbook.

## 8. Entrega

- Imagen inmutable, multi-stage, runtime no-root, sin toolchain (R-CD-14).
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  tests y cobertura por umbral en CI.
- Un servicio nuevo entra en: manifiesto, compose del entorno, matriz de CI
  (derivada), ruta del gateway, capacidades de `identity` y catálogo generado.
- Promoción `dev → staging → producción` por PR, no rebuild (R-GE-1).

## 9. Definition of Done de un servicio

Antes de considerar un servicio “en producción”:

- [ ] ADR del bounded context y de la decisión de crearlo.
- [ ] `service.json` validado y catálogo generado.
- [ ] Crate Rust con perfil y módulos mínimos.
- [ ] Esquema propio migrado, RLS y manifiesto de hashes.
- [ ] `/health`, `/ready`, `/metrics`, logs JSON y `x-request-id`.
- [ ] OpenAPI público/interno generado y sincronizado.
- [ ] Eventos declarados; outbox/consumidor idempotente si aplica.
- [ ] Capacidad registrada en `identity` y gating probado.
- [ ] Ruta y política en el gateway (si es público).
- [ ] Compose dev/stage/prod y matriz de CI.
- [ ] Tests unitarios, integración API+BD y contrato.
- [ ] SLO, alertas y runbook.
- [ ] README del paquete y entrada en el catálogo.
- [ ] Deuda nueva: cero; si hay excepción, baseline justificado y ADR.

> **Servicios existentes vs. nuevos:** la deuda congelada en
> `architecture-baseline.json` (15 servicios sin `/ready`, 6 lecturas de entorno,
> 13 acoplamientos de esquema) es deuda histórica con plan de reducción. Un
> servicio **nuevo** no puede nacer con ninguna de esas excepciones: el baseline
> no admite entradas nuevas (R-AF-2).

## 10. Anti-patrones prohibidos

| Anti-patrón | Por qué | Alternativa |
|---|---|---|
| Servicio por vertical (`farmacia-service`) | Multiplica el código y rompe el modelo por capacidad | Capacidad + pack + configuración (ADR-0001) |
| `JOIN` entre esquemas de servicios | Acopla despliegues y datos | API interna o proyección por eventos |
| Llamada HTTP síncrona para cobrar/reservar | Acopla disponibilidad y transacciones | Saga/outbox |
| Publicar al broker dentro de la transacción sin outbox | Pierde eventos o publica sin commit | Outbox transaccional |
| Consumidor no idempotente | Duplicados con entrega at-least-once | `eventos_procesados` |
| `/health` que consulta la BD | Reinicios innecesarios y falsos negativos | `/ready` para dependencias |
| Estado de negocio en el filesystem | El contenedor es efímero | PostgreSQL/objeto storage |
| `unwrap()`/`panic!` en ruta de request | Tumba el proceso | `Result` + `ApiError` |
| Leer `env::var` en el dominio | Configuración dispersa | `config.rs` + `AppState` |
| Regla sólo en un wiki | Se degrada sin aviso | Fitness function + baseline |
