# 01 — Reglas de Unit Tests

Aplica a: `*.spec.ts` de utils, value-objects, entidades, factories, servicios de dominio,
use-cases, controllers (lógica), pipes/guards/interceptors, componentes y servicios de Angular.

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
| R-U-9 | Cualquier parser de entrada externa (query params, CSV, payloads ZK) debe tener test de input malicioso/corrupto (`';--`, `<script>`, JSON roto, Buffer corrupto). |

## Controllers y casos de uso (mocks)

| ID | Regla |
|---|---|
| R-U-10 | El mock **no sustituye** la asertación de dominio. En controller: además de "se llamó al repo", verificar que los **valores construidos** (fechas, filtros, DTOs) son correctos según el requisito. |
| R-U-11 | Prohibido el test que solo verifica que un mock fue llamado sin validar argumentos semánticos (p.ej. `expect(repo.find).toHaveBeenCalled()` solo). |
| R-U-12 | Todo use-case debe tener tests de: camino feliz, validación de precondición (error), y propagación de error del repositorio. |
| R-U-13 | Respuestas de error: assert de **código HTTP + mensaje/shape**, no solo `rejects.toThrow()`. |

## Componentes frontend (Angular)

| ID | Regla |
|---|---|
| R-U-14 | Todo componente shared con lógica (emisión de eventos, formularios, inputs condicionales) tiene spec con interacción simulada (`click`, `emit`, `ngModel`). |
| R-U-15 | Pipes, guards e interceptors: test de cada rama (`canActivate` true/false, interceptor con/sin token, 401 vs otro error). |
| R-U-16 | Los specs no deben importar symbols inexistentes ni usar APIs privadas de componentes (evita specs rotos). |

## Cobertura de archivos unit

| ID | Regla |
|---|---|
| R-U-17 | **100% de archivos** en `domain/` (entidades, VOs, factories, services) y `core/shared/utils/` deben tener spec. Son código puro, sin excusa. |
| R-U-18 | Todo use-case nuevo agrega su spec en el mismo PR. |
