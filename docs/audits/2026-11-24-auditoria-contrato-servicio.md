# Auditoría de arquitectura — contrato de servicio

- **Fecha:** 2026-11-24
- **Base auditada:** `main` @ `312415df` (promoción `staging → main`, PR #850; árbol idéntico a `staging` `b25e0781` y `develop` `6221edfc`).
- **Alcance:** los 16 servicios de `packages/*`, sus manifiestos `service.json`, reglas `R-MS-9..16`, `R-AF-1..5`, composes, CI, esquemas PostgreSQL y documentación derivada.
- **Método:** ejecución de las fitness functions + lectura estática de los puntos que el baseline declara como deuda + contraste de eventos, perfiles y dependencias. **No** se levantó el stack completo para esta auditoría (el smoke E2E de stage se ejecutó antes de la promoción: run `45477519288`, success). **No** se hizo pentest ni prueba de carga.
- **Regla del documento:** cada hallazgo lleva evidencia reproducible. Lo que no se pudo verificar se declara en la sección 8.

---

## 1. Veredicto

> **El contrato de servicio está implantado y es ejecutable.** Los 16 servicios
> declaran manifiesto, los perfiles se cumplen, la matriz de CI se deriva de los
> manifiestos y la documentación no se edita a mano. **No hay violaciones duras**
> de `services-contract`.
>
> **La arquitectura todavía no es plenamente autónoma.** Hay **34 deudas
> congeladas**, y cuatro de ellas son **escrituras directas al esquema de otro
> servicio**, que rompen la propiedad de datos (R-DB-1/R-MS-16) y acoplan
> despliegues. Además, **15 de 16 servicios con base de datos no tienen
> `/ready`**, por lo que un orquestador no puede retirarlos de tráfico cuando
> PostgreSQL o RabbitMQ fallan.

La base es sólida: outbox transaccional, idempotencia, DLQ, envelope, contratos
OpenAPI, migraciones con hash, RLS y CI con 16 matrices Rust + 16 imágenes. El
problema ya no es “falta arquitectura”, sino **cerrar la deuda de propiedad de
datos y operabilidad** y **aumentar la cobertura ejecutable de las reglas
críticas** (hoy 67/254 IDs, 26,4 %).

### Scorecard

| Dimensión | Nota | Diagnóstico |
|---|---:|---|
| Manifiesto y catálogo (R-MS-9/R-MS-13) | **4,0** | 16/16 manifiestos válidos; owner/SLO nominales y genéricos. |
| Perfiles y módulos (R-MS-10) | **4,5** | 16/16 cumplen módulos mínimos; hay módulos de outbox sin eventos. |
| Readiness y ciclo de vida (R-MS-11) | **1,0** | `/health` en todos; `/ready` sólo en `api-gateway`. |
| 12-factor y puerto (R-MS-12) | **4,5** | Bind, EXPOSE, PORT y stateless correctos; rate limit del gateway en memoria. |
| Contrato de eventos (R-MS-14) | **3,0** | Eventos declarados y con dueño; sin AsyncAPI/CloudEvents y con eventos sin consumidor. |
| Observabilidad (R-MS-15) | **4,0** | RED + JSON + `x-request-id`; falta tracing OTel y readiness. |
| Propiedad de datos (R-MS-16) | **1,5** | 13 acoplamientos, 4 de ellos **escrituras** cross-schema. |
| Fitness functions (R-AF-1..4) | **3,5** | Motor sólido y docs generadas; cobertura ejecutable 67/254. |
| CI/CD y promoción (R-AF-3/R-GE) | **4,5** | Matriz derivada; promociones por delta verificadas. |

**Promedio ponderado: ≈ 3,4 / 5.** Contrato listo; autonomía de datos y
operabilidad, no.

---

## 2. Evidencia ejecutada

| Comando / verificación | Resultado |
|---|---|
| `npm run rules:check` | ✅ 24 suites / 101 tests / 0 fallos; **67/254 IDs** con check |
| `npm run architecture:check` | ✅ catálogo, matriz de eventos y deuda sincronizados |
| `npm run baseline:guard` | ✅ sin retroceso de baselines |
| `npm run test:engine` | ✅ 21 tests |
| `npm run test:smoke` | ✅ 79 tests |
| `npm run lint` | ✅ sin errores |
| `npm run promotion:check -- --source origin/staging --destination origin/main` | ✅ árbol idéntico |
| CI `develop` `6221edfc` | ✅ success |
| CI `staging` `b25e0781` | ✅ success |
| Stage Images `b25e0781` | ✅ success (16 servicios) |
| Deploy stage + smoke E2E `b25e0781` | ✅ success |
| CI `main` `312415df` | ✅ success (verificado tras el merge; PR #850 con 38 checks verdes) |

---

## 3. Hallazgos priorizados

| ID | Prioridad | Hallazgo | Evidencia | Regla |
|---|---|---|---|---|
| A1 | **P1** | `inventory` escribe `item.item_series` (marca vendido/devuelto) | `packages/inventory-service/src/reservas/mod.rs:204`, `reservas/reintegros.rs:196,281` | R-MS-16/R-DB-1 |
| A2 | **P1** | `pricing` escribe `item.items.tipo_precio` | `packages/pricing-service/src/precios/escalas.rs:108`, `precios_items.rs:294` | R-MS-16/R-DB-1 |
| A3 | **P1** | `identity` inserta `inventory.ubicaciones` en el alta de empresa/negocio | `packages/identity-service/src/dominio/empresas/creacion.rs:187`, `dominio/negocios.rs:518` | R-MS-16/R-DB-1 |
| A4 | **P1** | `identity` actualiza `finance.configuracion_fiscal` al cambiar de región | `packages/identity-service/src/dominio/regiones.rs:223` | R-MS-16/R-DB-1 |
| A5 | **P1** | 15 servicios con BD sin `/ready`; `/health` es liveness puro | `architecture-baseline.json` (15 entradas `readiness:*:ready`) | R-MS-11 |
| A6 | **P2** | Lecturas cross-schema en 9 servicios (informes, joins de identidad/catálogo) | 13 entradas `cross-schema` del baseline; detalle en 5.8 | R-MS-16 |
| A7 | **P2** | 6 archivos leen entorno fuera de `config.rs`/`db.rs` | `item-service/src/productos/{mod,helpers}.rs`; `orders-service/src/{pagos/configuracion,pagos/webhooks,pasarelas/pagadito,pasarelas/tilopay}.rs` | R-MS-5 |
| A8 | **P2** | Owner, SLO y runbook son nominales: los 16 servicios declaran `core-platform`, 99,5 %/1.500 ms y el mismo runbook genérico | `packages/*/service.json` | R-MS-13 |
| A9 | **P2** | Contrato de eventos sin AsyncAPI/CloudEvents; envelope propio `{tipo,event_id,data}` | `packages/README.md` §6, `docs/architecture/01` §3.3 | R-MS-14 |
| A10 | **P2** | Cobertura ejecutable baja en reglas críticas: R-AR-2 (sin mutación síncrona), R-AR-3/4 (outbox/DLQ), R-DI-2..5 (stock/estado/idempotencia/partida doble), R-DB-2..5 (RLS/transacción/índices/soft delete) | `rules:check` 67/254; mapa por área | R-AF-1 |
| A11 | **P3** | `analytics-service` declara `OUTBOX_TABLA` con default `"outbox"` y test `intelligence.outbox`, pero no publica eventos | `packages/analytics-service/src/config.rs:74,125,132` | R-MS-9/R-MS-14 |
| A12 | **P3** | `field-service` dice “no usa RabbitMQ ni outbox”, pero los tres composes fijan `OUTBOX_TABLA: field.outbox` | `packages/field-service/src/config.rs:73`; `docker-compose*.yml` | R-MS-9 |
| A13 | **P3** | `item` y `pricing` mantienen `outbox.rs` y relay sin publicar eventos | `packages/item-service/src/main.rs`, `packages/pricing-service/src/main.rs` | R-MS-10/R-MS-14 |
| A14 | **P3** | 8 eventos publicados sin consumidor (`appointment.*`, `asiento.registrado`, `comprobante.emitido`, `declaracion.generada`, `storage.objeto.*`) | `docs/architecture/matriz-eventos.md` | R-MS-14 |
| A15 | **P3** | Rate limit del gateway es en memoria por instancia; con réplicas se multiplica el límite | `packages/api-gateway/src/rate_limit.rs` | R-DS-6/R-MS-12 |

---

## 4. Detalle por regla

### 4.1 R-MS-9 — Manifiesto

- 16/16 `service.json` válidos; `SERVICES`, puertos y umbrales derivan de ellos.
- `metadata.name`, crate, binario, `EXPOSE`, `ENTRYPOINT` y compose coinciden.
- **Gaps:** A11 (config de analytics), A12 (field/outbox), A13 (outbox sin
  eventos). No son fallos de conformidad porque el manifiesto describe el
  contrato, pero son señales de código muerto o copiado.

### 4.2 R-MS-10 — Perfiles y módulos

| Perfil | Servicios | Módulos mínimos | Estado |
|---|---|---|---:|
| `edge` | `api-gateway` | auth/config/errors/state/logs/metrics/http/policy/proxy/rate_limit/readiness | ✅ |
| `core` | `identity`, `item`, `pricing`, `field` | + `db` | ✅ |
| `transactional` | `inventory`, `orders`, `logistics`, `scheduling`, `commissions`, `finance`, `storage` | + `rabbit`/`outbox` | ✅ |
| `consumer` | `stores`, `notifications` | + `rabbit` | ✅ |
| `olap` | `market-intelligence`, `analytics` | + `rabbit` y proyecciones | ✅ |

No hay un módulo compartido de runtime; la duplicación es deliberada (R-AR-5).

### 4.3 R-MS-11 — Salud y readiness

- `/health` existe en los 16 y es liveness (no consulta BD).
- `/ready` sólo existe en `api-gateway`
  (`packages/api-gateway/src/http/mod.rs:42`).
- Los 15 servicios con BD están en el baseline. El healthcheck de compose
  (`docker-compose.yml:38-42`) apunta a `/health`, así que **no detecta** una BD
  caída. Riesgo: tráfico a un servicio que no puede atender.
- **Remediación:** implementar `/ready` con `SELECT 1` al pool y ping al broker
  (si aplica); devolver `503` con código estable. Empezar por `orders`,
  `inventory`, `identity`, `finance`; luego el resto.

### 4.4 R-MS-12 — 12-factor y puerto

- `0.0.0.0:<PORT>`, `EXPOSE`, `PORT` y `spec.port` coinciden en 16/16.
- Sin escrituras a filesystem en `src/` (check `stateless`).
- **Observación:** el rate limit del gateway es un `Mutex<HashMap>` en memoria;
  con más de una réplica el límite efectivo se multiplica. No viola 12-factor
  (el estado es efímero), pero sí la intención de R-DS-6 en escala horizontal.

### 4.5 R-MS-13 — Catálogo, ownership y SLO

- Los 16 declaran `owner: core-platform`, `lifecycle: production`, SLO
  99,5 %/1.500 ms y `docs.readme`/`openapi`.
- **Gap:** no hay owner por dominio ni runbook/on-call por servicio. El catálogo
  es correcto como contrato, pero no permite responder “¿a quién llamo?” sin
  mirar CODEOWNERS (que hoy apunta a una sola persona).
- `commissions-service` declara `lifecycle: production` aunque sólo se despliega
  en dev/stage (`spec.environments`). Es coherente con “madurez”, pero conviene
  aclararlo en el README del servicio.

### 4.6 R-MS-14 — Eventos

- 24 eventos en la matriz: 23 con publisher local y 1 externo
  (`venta.geolocalizada`).
- 8 eventos publicados sin consumidor (A14): no rompen la regla (pueden ser
  integración saliente), pero son contrato sin uso; documentar o retirar.
- **Gap:** no hay AsyncAPI ni CloudEvents. El envelope propio funciona y está
  probado, pero un consumidor nuevo debe leer código para conocer el contrato.
  Próximo paso: AsyncAPI 3.0 por servicio + CloudEvents como envelope objetivo.
- Idempotencia: los 9 consumidores usan `<schema>.eventos_procesados`
  (`inventory`, `orders`, `logistics`, `commissions`, `finance`, `stores`,
  `notifications`, `analytics`, `intelligence`). Los publicadores usan
  `insertar_en_transaccion` (27 usos) y relay con `FOR UPDATE SKIP LOCKED`.

### 4.7 R-MS-15 — Observabilidad

- 16/16 exponen `/metrics`, registran RED (`core_engine_http_requests_total`,
  `core_engine_http_request_duration_seconds`) y logs JSON.
- `x-request-id` presente en todos; se propaga por outbox/RabbitMQ.
- **Gap:** no hay OpenTelemetry/tracing distribuido; la correlación es por
  `x-request-id`, suficiente para el piloto pero no para latencias por span.
- `/ready` faltante impacta directamente la operabilidad (A5).

### 4.8 R-MS-16 — Propiedad de datos (hallazgo principal)

**Escrituras cross-schema (P1):**

| Servicio | Escribe en | Evidencia | Riesgo |
|---|---|---|---|
| `inventory` | `item.item_series` | `reservas/mod.rs:204`, `reservas/reintegros.rs:196,281` | `item` no controla el ciclo de series; rompe la autonomía y puede pisar reglas de catálogo. |
| `pricing` | `item.items.tipo_precio` | `precios/escalas.rs:108`, `precios_items.rs:294` | `item` es dueño del ítem; `pricing` cambia un flag de catálogo. |
| `identity` | `inventory.ubicaciones` | `empresas/creacion.rs:187`, `negocios.rs:518` | Alta de tenant escribe en inventario sin pasar por su API/evento. |
| `identity` | `finance.configuracion_fiscal` | `regiones.rs:223` | Cambio de región fiscal escribe en finanzas. |

**Lecturas cross-schema (P2):**

| Servicio | Lee | Evidencia |
|---|---|---|
| `identity` | `finance.jurisdicciones` | `packs/escritura.rs:143,337,389`, `packs/lectura.rs:110,167,218`, `regiones.rs:497,508` |
| `item` | `identity.negocios`, `inventory.ubicaciones` | `items/escritura.rs:19,32`, `series/mod.rs:339` |
| `inventory` | `item.items`, `item.item_series`, `item.item_componentes`, `orders.orden_vista` | `inventario/consultas.rs`, `reservas/*`, `lifecycle.rs:67` |
| `pricing` | `item.items` | `precios/consulta.rs:192`, `promociones/mod.rs:279,289`, `tarifas/mod.rs:281` |
| `stores` | `identity.asignaciones_personal`, `identity.negocios` | `ofertas/mod.rs:315,330,402,537`, `tiendas/*` |
| `orders` | `identity.partes`, `identity.negocios` | `fidelizacion/tarjetas.rs:14,37,62,150,193`, `views.rs:210` |
| `commissions` | `orders.orden_vista`, `identity.negocios` | `lifecycle.rs:73,113`, `reportes.rs:143` |
| `finance` | `identity.partes`, `identity.negocios`, `identity.empresas` | `comercio/creditos.rs`, `contabilidad.rs:593`, `facturacion/emision.rs:31,435`, `tributacion/jurisdicciones.rs:49` |
| `analytics` | `identity.negocios`, `identity.empresas` | `analitica.rs:52,367` |

**Remediación por caso:**

1. `inventory → item.item_series`: `inventory` emite `serie.reservada`/
   `serie.liberada` (o `stock.*` ya existente) y `item` consume y actualiza su
   serie; alternativa transitoria: API interna `item /internal/series/:id/estado`.
2. `pricing → item.items.tipo_precio`: mover el flag a `pricing` (tabla propia de
   configuración por ítem) o emitir `pricing.escala.activada` para que `item` lo
   proyecte.
3. `identity → inventory.ubicaciones`: emitir `empresa.creada`/`negocio.creado`
   (ya existe el patrón) y que `inventory` cree la ubicación predeterminada.
4. `identity → finance.configuracion_fiscal`: emitir
   `empresa.region_tributaria_cambiada` (ya existe) y que `finance` actualice su
   configuración al consumirlo.
5. Lecturas: reemplazar por proyecciones locales alimentadas por los eventos que
   ya existen (`negocio.creado`, `empresa.*`, `stock.updated`, `order.*`) o por
   API interna de solo lectura con timeout y degradación.

### 4.9 R-AF-1..4 — Fitness functions

- 24 suites / 101 tests / 67 de 254 IDs (26,4 %).
- **Bien cubierto:** manifiesto, perfiles, artefactos, compose, esquema,
  módulos, arranque, observabilidad, contrato OpenAPI, migraciones, RLS,
  secretos, SQLi/SSRF/uploads, tamaño de módulos, entrega.
- **Sin check ejecutable (críticas):**
  - `R-AR-2` (mutación crítica por HTTP síncrono).
  - `R-AR-3` (outbox en la misma transacción; el check actual sólo verifica que
    exista `outbox.rs`).
  - `R-AR-4` (DLQ/backoff).
  - `R-DI-2` (stock atómico), `R-DI-3` (máquina de estados), `R-DI-4`
    (idempotencia), `R-DI-5` (partida doble).
  - `R-DB-2..5` (RLS, transacción, índices, soft delete).
  - `R-CD-9..11` (entornos Dockploy, gate de producción).
  - `R-GW-4..8` (TTL, cookies, errores, versionado, API keys).
- **Recomendación:** añadir checks de alto valor y bajo coste estático:
  (a) detectar `reqwest`/`Client` en rutas de mutación sin outbox;
  (b) exigir `FOR UPDATE`/`CHECK` en SQL de stock;
  (c) exigir `eventos_procesados` en cada consumidor (ya se puede);
  (d) exigir `FOR UPDATE SKIP LOCKED` en el relay de outbox;
  (e) validar `R-GW-7` (todas las rutas públicas bajo `/api/v2`).

---

## 5. Plan de remediación

### Fase 1 — P1 (autonomía y operabilidad)

1. **Eliminar las 4 escrituras cross-schema** (A1–A4). Es el cambio de mayor
   impacto arquitectónico y el que más reduce el acoplamiento de despliegues.
2. **Añadir `/ready`** a `orders`, `inventory`, `identity`, `finance` y luego al
   resto (A5). Retirar cada entrada del baseline en el mismo PR.
3. **Verificar CI de `main`** y dejar el run verde como evidencia de la
   promoción.

### Fase 2 — P2 (contrato y gobierno)

4. Mover las 6 lecturas de entorno a `config.rs` (A7).
5. Asignar owner/runbook/SLO por dominio y declarar `lifecycle` real (A8).
6. Publicar AsyncAPI 3.0 por servicio y evaluar CloudEvents (A9).
7. Añadir las fitness functions críticas de R-AR/R-DI/R-DB (A10).
8. Reemplazar lecturas cross-schema por proyecciones/API interna (A6).

### Fase 3 — P3 (limpieza)

9. Corregir `analytics` (A11), `field` (A12) y retirar outbox sin eventos (A13).
10. Documentar o retirar eventos sin consumidor (A14).
11. Evaluar rate limit distribuido en el gateway (A15).

### Criterio de salida de la auditoría

- 0 escrituras cross-schema.
- `/ready` en los 16 servicios con BD y baseline `readiness` vacío.
- `architecture-baseline.json` con sólo deuda de lectura cross-schema o vacío.
- Cobertura ejecutable ≥ 120/254 IDs, priorizando R-AR-2..4, R-DI-2..5 y
  R-DB-2..5.
- AsyncAPI versionado y validado en CI.

---

## 6. Anexo — Matriz de perfiles

| Servicio | Perfil | BD | Broker | Outbox | Eventos publicados |
|---|---|---:|---:|---:|---:|
| `api-gateway` | edge | — | — | — | 0 |
| `identity-service` | core | ✅ | — | — | 0 |
| `item-service` | core | ✅ | — | ⚠️ sin uso | 0 |
| `pricing-service` | core | ✅ | — | ⚠️ sin uso | 0 |
| `field-service` | core | ✅ | — | ⚠️ compose | 0 |
| `stores-service` | consumer | ✅ | ✅ | — | 0 |
| `notifications-service` | consumer | ✅ | ✅ | — | 0 |
| `inventory-service` | transactional | ✅ | ✅ | ✅ | 4 |
| `orders-service` | transactional | ✅ | ✅ | ✅ | 7 |
| `logistics-service` | transactional | ✅ | ✅ | ✅ | 2 |
| `scheduling-service` | transactional | ✅ | ✅ | ✅ | 3 |
| `commissions-service` | transactional | ✅ | ✅ | ✅ | 2 |
| `finance-service` | transactional | ✅ | ✅ | ✅ | 3 |
| `storage-service` | transactional | ✅ | ✅ | ✅ | 2 |
| `market-intelligence-service` | olap | ✅ | ✅ | — | 0 |
| `analytics-service` | olap | ✅ | ✅ | — | 0 |

---

## 7. Lo que está bien y no hay que reabrir

- 16/16 servicios con manifiesto válido y contrato ejecutable.
- Matriz de CI derivada; no hay listas paralelas de servicios.
- Outbox transaccional e idempotencia en los 9 consumidores.
- DLQ/reintentos en los servicios con broker.
- OpenAPI sincronizado y envelope uniforme.
- Promoción por delta `R-GE-7` verificada en las tres ramas.
- Documentación generada y sin drift (`architecture:check`).

---

## 8. No verificado en esta auditoría

- CI de `main` `312415df`: verificado en success después del merge; el PR #850
  tenía 38 checks verdes y el árbol es idéntico a `staging`, cuyo CI y smoke E2E
  pasaron.
- Carga real, caos, latencia p95 y comportamiento multi-réplica.
- RLS efectivo en un clúster productivo (el baseline de RLS es estático).
- Pentest y revisión de seguridad de perímetro (fuera del alcance de estas
  reglas; ver auditoría 2026-11-20).
- Contratos AsyncAPI (no existen todavía).
