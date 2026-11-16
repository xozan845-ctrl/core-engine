# 01 — Reglas de Unit Tests

Aplica a: `mod tests` de utils, value-objects, entidades, factories, dominio y
consumidores (lógica), handlers HTTP y middleware/guards del gateway.

## Semántica del requisito

| ID | Regla |
|---|---|
| R-U-1 | **Assert contra el requisito, no contra la implementación.** Si cambia la implementación pero el requisito no, el test no debe cambiar (y viceversa). Prohibido "actualizar el expected para que pase". |
| R-U-2 | Nombre de test en formato `debe <comportamiento> cuando <condición>` (o `should ... when ...`). Nunca `should work` / `test 1`. |
| R-U-3 | Cada test cubre **una** situación. Sin "y" concatenando escenarios en un solo `it`. |
| R-U-4 | El test debe poder fallar: si la lógica bajo prueba se elimina, el test falla. Nada de asserts triviales (`expect(true).toBe(true)`, `expect(x).toBeDefined()` como único assert). |

## Fechas, zonas horarias y números

| ID | Regla |
|---|---|
| R-U-5 | Cualquier lógica de fecha/hora debe testearse **con la timezone explícita** (parámetro o `process.env.TZ` fijado en el test). Prohibido depender de la timezone de la máquina. |
| R-U-6 | Incluir casos boundary de fecha: cambio de día UTC vs local (ej. `04:59:59Z` vs `05:00:00Z` en UTC-5), fin de mes, año bisiesto, DST si aplica. |
| R-U-7 | Inputs numéricos: probar 0, negativos, `NaN`, `Infinity`, límites de tipo. |

## Inputs inválidos (defensa)

| ID | Regla |
|---|---|
| R-U-8 | Toda función que acepta `string \| null \| undefined` debe testear: `undefined`, `null`, `''`, `'   '`, `'undefined'`, `'null'`, string inválido. |
| R-U-9 | Cualquier parser de entrada externa (query params, CSV, payloads de mensajería o de llegada por el gateway) debe tener test de input malicioso/corrupto (`';--`, `<script>`, JSON roto, Buffer corrupto). |

## Controllers y casos de uso (mocks)

| ID | Regla |
|---|---|
| R-U-10 | El mock **no sustituye** la asertación de dominio. En un handler HTTP o caso de uso: además de "se llamó al puerto/repositorio", verificar que los **valores construidos** (fechas, filtros, montos, campos normalizados) son correctos según el requisito. |
| R-U-11 | Prohibido el test que solo verifica que un doble fue llamado sin validar argumentos semánticos (p.ej. un `assert!(mock.fue_llamado())` sin revisar el filtro o el monto). |
| R-U-12 | Todo caso de uso (función de dominio o consumidor con lógica) debe tener tests de: camino feliz, validación de precondición (error) y propagación del error de base de datos o del puerto. |
| R-U-13 | Respuestas de error: assert de **código HTTP + `codigo`/mensaje/shape** (`ApiError`), no solo que la función devuelva `Err`. |

## Frontend (N/A hoy) y guardas/pipes/interceptors

| ID | Regla |
|---|---|
| R-U-14 | **N/A hoy — sin frontend en el repo** (G-2 y R-COV-5 **reservados**, nunca reutilizados). El día que exista UI: todo componente con lógica (emisión de eventos, formularios, inputs condicionales) tiene spec con interacción simulada (`click`, `emit`, `ngModel`). |
| R-U-15 | Middleware y guards del gateway: test de cada rama (con/sin token, rol insuficiente, cabecera interna, límite de tasa y mapeo de errores 401 vs 403 vs 500). |
| R-U-16 | Los specs no deben importar symbols inexistentes ni usar APIs privadas de la implementación (evita specs rotos). |

## Cobertura de archivos unit

| ID | Regla |
|---|---|
| R-U-17 | **Código puro con `mod tests`**: las funciones puras de cada crate (validaciones, cálculo de montos, normalización, FEFO, transiciones de estado) llevan pruebas unitarias en el mismo archivo. La infraestructura (`db.rs`, `rabbit.rs`, `outbox.rs`, `metrics.rs`, `logs.rs`) se cubre con las suites de integración del crate, no con mocks de broker. |
| R-U-18 | Todo caso de uso, handler o consumidor nuevo agrega sus pruebas en el mismo PR (`mod tests` o `tests/<dominio>_*.rs`). |
