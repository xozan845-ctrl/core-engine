# 03 — Conformidad arquitectónica (fitness functions)

Aplica a: la gobernanza de la arquitectura, no a un servicio concreto. Define
cómo una decisión arquitectónica se convierte en una **fitness function**
ejecutable, cómo se congela la deuda existente y cómo se evita que la
arquitectura se degrade por cambios silenciosos.

> Fundamento: *Building Evolutionary Architectures* (Ford, Parsons, Kua) y
> *fitness function-driven development* (Thoughtworks). Una regla que sólo vive
> en un documento no protege nada; si es verificable por máquina, se automatiza.

## Reglas

| ID | Regla |
|---|---|
| R-AF-1 | **Toda propiedad arquitectónica verificable tiene una fitness function**: si una regla de `docs/rules/` puede comprobarse con inspección estática, contrato o test, DEBE existir una suite en `scripts/rules/` que la cite por ID y esté registrada en `scripts/config/manifest.mjs` (R-CI-7). La suite y la regla entran en el **mismo PR**. Una propiedad sin check se marca explícitamente como “no automatizable” en el README del área. |
| R-AF-2 | **La deuda se congela, no se ignora**: cuando una regla nueva encuentra violaciones preexistentes, éstas se enumeran en `scripts/config/architecture-baseline.json` con una clave estable (`check:servicio:detalle`) y una justificación. El baseline **sólo puede reducirse**: `npm run baseline:guard` rechaza cualquier entrada nueva, y la suite falla si una entrada quedó obsoleta (se corrigió el hallazgo pero no se retiró del baseline). No se admite congelar reglas duras (artefactos, identidad, esquema, arranque, eventos, documentación). |
| R-AF-3 | **Fuente única de verdad para el inventario**: la lista de servicios, puertos, perfiles, umbrales de cobertura y la matriz de CI se derivan de `packages/<servicio>/service.json` (R-MS-9). Prohibido duplicar esa lista en `project.mjs`, workflows, composes o documentación generada. Un servicio nuevo debe aparecer en el catálogo, el compose, el CI y el contrato sin editar listas paralelas. |
| R-AF-4 | **La documentación derivada se genera y se verifica**: catálogo de servicios, matriz de eventos y deuda arquitectónica se generan con `npm run architecture:report` desde los manifiestos. CI ejecuta `npm run architecture:check` y falla si el resultado difiere del versionado. Un documento derivado nunca se edita a mano (lleva cabecera `GENERADO`). |
| R-AF-5 | **Una decisión arquitectónica nueva exige ADR y enmienda de reglas**: cambiar perfiles, contratos de comunicación, propiedad de datos, envelope de eventos o el modelo de despliegue requiere un ADR en `docs/adr/` con contexto, decisión, consecuencias y criterios de aceptación; el mismo PR actualiza la regla afectada y su fitness function. Prohibido cambiar el comportamiento por edición silenciosa de un baseline o de un workflow. |

## Anatomía de una fitness function

Toda entrada del catálogo de conformidad declara:

| Elemento | Pregunta | Ejemplo |
|---|---|---|
| Característica | ¿Qué atributo protege? | Aislamiento de datos (R-DB-1) |
| Métrica | ¿Qué evidencia lo demuestra? | Nº de sentencias SQL contra un esquema ajeno |
| Umbral | ¿Qué valor es aceptable? | 0 nuevas; deuda existente congelada |
| Disparador | ¿Cuándo se ejecuta? | Cada PR (`rules:check`) |
| Respuesta | ¿Qué pasa si falla? | Build rojo con servicio, archivo y remediación |

## Conformidad actual

| Suite | IDs | Qué verifica |
|---|---|---|
| `services-contract` | R-MS-9..16, R-AF-1..3 | Manifiesto, perfil, módulos, compose, esquema, arranque, observabilidad, eventos, documentación |
| `services-monorepo` | R-CD-3/4/14 | Artefactos de crate e imagen |
| `microservices-ci-contract` | R-MS-1, R-CI-5, R-COV-1/4 | Inventario y matriz de CI derivados de los manifiestos |
| `microservices-tamano-modulos` | R-MS-7 | Presupuesto de tamaño y funciones por archivo |
| `architecture-agnosticismo` | R-AR-5 | Sin acoplamiento a un vertical concreto |

## Verificación

```bash
npm run rules:check            # incluye services-contract
npm run architecture:check     # catálogo/matriz/deuda sincronizados
npm run baseline:guard         # la deuda no retrocede
npm run test:engine            # el motor sigue siendo fiable
```
