# 05 — Reglas de Robustez y Calidad

Reglas transversales que suben la calidad más allá del happy-path.

## Regression-first (la regla que motivó este documento)

| ID | Regla |
|---|---|
| R-REG-1 | **Todo bug confirmado**: primero se agrega el test que FALLA con el bug (rojo), luego la fix (verde). El PR del fix no se acepta sin ese test. El test se marca o comenta como `regression: <id-issue>`. |
| R-REG-2 | El test de regresión aserta el **requisito correcto**, no "el cambio que hicimos" (ver R0). |
| R-REG-3 | Después de un bug de una capa (ej. unit), se evalúa si faltan capas (integración/E2E) que lo hubieran detectado antes; si sí, se agrega al menos un test en esa capa (ver `AUDIT.md` → plan). |

## Property-based testing

| ID | Regla |
|---|---|
| R-PB-1 | Funciones puras de transformación (parsers de fecha, conversores, cálculos de dinero/comisiones, exportadores CSV) deben tener **property tests**: invariantes generadas con cientos de inputs aleatorios. Implementado con `fast-check` en `packages/shared/src/*.property.spec.ts` para dinero y ciclo de la orden. |
| R-PB-2 | Propiedades mínimas para fechas: `parse(format(x))` conserva el día local; inicio de rango ≤ fin de rango; `dateFrom=dateTo=D` solo devuelve registros del día local D (monotonicidad y partición: suma de días = total del rango). |
| R-PB-3 | Propiedades de dinero y ciclo de la orden: invariantes de consistencia (centavos siempre enteros y ≥ 0, comisión = rate × base exacta, transiciones de estado solo por la máquina permitida — `validarTransicion` —, idempotencia de reejecución sobre los mismos eventos). `Money` rechaza centavos negativos y normaliza `-0`; los property tests cubren round-trip, suma/resta, márgenes, comisión, formato y todas las transiciones. |

## Mutation testing

| ID | Regla |
|---|---|
| R-MT-1 | Módulos críticos (`identity`/auth, `orders` (CQRS + event sourcing), `commissions`/`finance` (dinero)) se someten a mutation testing (Stryker) en periodicidad mensual o pre-release. Implementado en `.github/workflows/mutation.yml` (cron mensual + `workflow_dispatch`) para `shared`, `identity-service`, `orders-service`, `commissions-service` y `finance-service`; no es gate de PR. |
| R-MT-2 | **Rangos de mutantes eliminados: ≥ 90% para módulos críticos** (`identity`, `orders`, `commissions`/`finance` — gate `break` de Stryker), ≥ 80% para cualquier otro módulo sometido a mutación, y por debajo de **70% el release queda bloqueado** (piso absoluto). Mutantes sobrevivientes → tests débiles a corregir antes del release. **Baseline 2026-10-20:** `shared` **96.95%** (money 98.10%, order-state 92.31%), `orders-service` **92.13%** (`create-order-command.handler.ts`) e `identity-service` **100%** (`auth.service.ts`), con `break: 90` activo en los tres. `commissions` y `finance` siguen pendientes de baseline. El runner usado es `command` (Vitest real sobre el sandbox) porque el runner de Vitest no aplicaba el mutation switching a `money.ts`; el JSON se publica como artefacto. |
| R-MT-3 | Un test que no mata ningún mutante (equivalentes descartados) se mejora o elimina. |

## Robustez de entradas y concurrencia

| ID | Regla |
|---|---|
| R-RB-1 | Fuzz mínimo en parsers externos (query params, CSV export/import, payloads de mensajería y bodies de llegada por el gateway): entradas truncadas, Unicode raro, campos de más, números como strings. Implementado con `fast-check` en `packages/shared/src/fuzz-parsers.spec.ts` para `parsearPaginacion`, `crearPagina` y `parametrizarRuta`. |
| R-RB-2 | Límites de rate/size: probar listados con `limit` gigante, búsqueda con string enorme, payload sobre el máximo permitido. |
| R-RB-3 | Outbox/saga: probar reentrada/overlap (dos reenvíos del outbox o dos eventos de la misma saga simultáneos) y comportamiento ante fallo parcial (¿quedan datos a medias?, ¿va a DLQ con backoff?). |
| R-RB-4 | Timeouts y caída de un servicio o del broker: el sistema degrada con error controlado, no excepción cruda. |

## Anti-flakiness

| ID | Regla |
|---|---|
| R-FL-1 | Cero tolerancia a tests intermitentes. Un test que falla 1/20 corre en CI: se arregla o se marca `quarantine` con issue y fecha máxima de reparación. |
| R-FL-2 | Prohibido `sleep` arbitrario; usar esperas activas (`waitFor` de condición) con timeout. |
| R-FL-3 | Sin dependencia de orden entre tests ni de timezone/máquina (fijar `TZ` en el setup de Jest). |
| R-FL-4 | Sin dependencia de red externa o de servicios de terceros (los que no levanta el `docker compose`). |
