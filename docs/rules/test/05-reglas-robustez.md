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
| R-PB-1 | Las funciones puras de transformación (parsers de fecha, conversores, cálculos de dinero/comisiones, exportadores CSV) que tengan invariantes algebraicos deben probarse con **property tests** (`proptest` como dev-dependency del crate): cientos de inputs generados contra la propiedad, no solo ejemplos elegidos a mano. Si una función no tiene invariantes, se justifica en el PR y se cubre con casos boundary. |
| R-PB-2 | Propiedades mínimas para fechas: `parse(format(x))` conserva el día local; inicio de rango ≤ fin de rango; `dateFrom=dateTo=D` solo devuelve registros del día local D (monotonicidad y partición: suma de días = total del rango). |
| R-PB-3 | Propiedades de dinero y ciclo de la orden: invariantes de consistencia (centavos siempre enteros y ≥ 0, comisión = rate × base exacta, transiciones de estado solo por la máquina permitida — `validarTransicion` —, idempotencia de reejecución sobre los mismos eventos). `Money` rechaza centavos negativos y normaliza `-0`; los property tests cubren round-trip, suma/resta, márgenes, comisión, formato y todas las transiciones. |

## Mutation testing

| ID | Regla |
|---|---|
| R-MT-1 | Los módulos críticos (`identity`/auth, `orders` (CQRS + event sourcing), `commissions`/`finance` (dinero)) exigen pruebas que **maten mutaciones de lógica**: cada rama de validación, cada transición de estado y cada fórmula de dinero tiene al menos un caso que falla si se invierte la condición, se cambia un operador o se altera un límite. La mutación se ejerce con property/unit tests en `mod tests`; el criterio no depende de una herramienta concreta. |
| R-MT-2 | **Umbral de calidad:** para módulos críticos, toda condición de negocio (autorización, montos, stock, estados) debe tener un caso positivo y uno negativo; una rama sin caso contrario no es aceptable aunque la cobertura de líneas sea alta. Los property tests de R-PB-3 cubren los invariantes de dinero y transiciones. Un test que solo recorre el camino feliz no cumple R-MT-1. |
| R-MT-3 | Un test que no puede fallar ante un cambio de la lógica (asserts triviales, mocks que se autoafirman) se mejora o se elimina. |

## Robustez de entradas y concurrencia

| ID | Regla |
|---|---|
| R-RB-1 | Fuzz mínimo en parsers externos (query params, CSV export/import, payloads de mensajería y bodies de llegada por el gateway): entradas truncadas, Unicode raro, campos de más, números como strings. Se cubre con casos de tabla en `mod tests` de cada parser y, cuando el parser lo permita, con `proptest`. |
| R-RB-2 | Límites de rate/size: probar listados con `limit` gigante, búsqueda con string enorme, payload sobre el máximo permitido. |
| R-RB-3 | Outbox/saga: probar reentrada/overlap (dos reenvíos del outbox o dos eventos de la misma saga simultáneos) y comportamiento ante fallo parcial (¿quedan datos a medias?, ¿va a DLQ con backoff?). |
| R-RB-4 | Timeouts y caída de un servicio o del broker: el sistema degrada con error controlado, no excepción cruda. Implementado en los clientes HTTP Rust (`reqwest` con timeout) y check ejecutable en `microservices-ci-contract`. |

## Anti-flakiness

| ID | Regla |
|---|---|
| R-FL-1 | Cero tolerancia a tests intermitentes. Un test que falla 1/20 corre en CI: se arregla o se marca `quarantine` con issue y fecha máxima de reparación. |
| R-FL-2 | Prohibido `sleep` arbitrario; usar esperas activas (`waitFor` de condición) con timeout. |
| R-FL-3 | Sin dependencia de orden entre tests ni de timezone/máquina (fijar `TZ` en los workflows y runners de integración/E2E). |
| R-FL-4 | Sin dependencia de red externa o de servicios de terceros (los que no levanta el `docker compose`). |
