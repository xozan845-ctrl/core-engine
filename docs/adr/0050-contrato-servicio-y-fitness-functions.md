# ADR-0050 — Contrato de servicio declarativo y fitness functions

- **Fecha:** 2026-11-24
- **Estado:** Aceptado (implementado)
- **Decisores:** arquitectura de plataforma
- **Relacionado:** ADR-0001 (multi-vertical), ADR-0002 (envelope), ADR-0003
  (outbox), ADR-0018 (split de catálogo), ADR-0022 (analytics), R-MS-9..16,
  R-AF-1..5

## Contexto

El repositorio tenía un motor de reglas sólido (`scripts/engine`,
`docs/rules/`), pero la arquitectura estaba descrita de forma fragmentada:

- reglas temáticas repartidas por áreas, con referencias a un stack anterior
  (NestJS, `@core/shared`, `QueryRunner`) y sin un contrato único de servicio;
- listas de servicios y umbrales de cobertura duplicados en `project.mjs`, en
  `.github/workflows/ci.yml` y en los composes;
- documentación derivada desactualizada (AUDIT con snapshots de Node, catálogo
  manual);
- deriva real no visible: servicios con base de datos sin `/ready`, lecturas de
  entorno fuera de `config.rs`, y SQL cross-schema (por ejemplo, `inventory`
  actualizando `item.item_series` o `analytics` leyendo `identity.negocios`);
- cobertura ejecutable baja (52/241 IDs) y sin política para congelar deuda sin
  ignorarla.

La consecuencia era predecible: cada servicio interpretaba la arquitectura a su
manera y las reglas no protegían las propiedades que decían proteger.

## Decisión

Adoptar un **contrato de servicio declarativo y ejecutable**:

1. **Manifiesto por servicio** `packages/<servicio>/service.json` (R-MS-9) con
   metadata (owner, lifecycle, bounded context) y spec (perfil, puerto, esquema,
   outbox, capacidades, dependencias, eventos, SLO, docs, cobertura).
2. **Perfiles arquitectónicos** `edge/core/transactional/consumer/olap` con
   módulos mínimos obligatorios (R-MS-10).
3. **Fitness functions** en `scripts/rules/`: la suite `services-contract`
   contrasta el manifiesto con crate, Dockerfile, compose, esquema, módulos,
   arranque, observabilidad, eventos y documentación (R-AF-1).
4. **Deuda congelada** en `scripts/config/architecture-baseline.json` con
   política de reducción monotónica; `baseline:guard` impide crecimiento y la
   suite detecta entradas obsoletas (R-AF-2).
5. **Fuente única de verdad**: `SERVICES`, puertos, perfiles, umbrales y matriz
   de CI se derivan de los manifiestos (R-AF-3). La matriz de CI se emite con
   `scripts/tools/ci-matrix.mjs` y se consume con
   `fromJSON(needs.matrix.outputs.include)`.
6. **Documentación derivada**: catálogo de servicios, matriz de eventos y deuda
   se generan con `npm run architecture:report`; CI falla si hay drift (R-AF-4).
7. **ADR obligatorio** para cambiar perfiles, contratos, propiedad de datos o
   modelo de despliegue (R-AF-5).

La declaración narrativa vive en `docs/architecture/`; la norma ejecutable en
`docs/rules/microservice/03-contrato-servicio.md` y
`docs/rules/architecture/03-conformidad.md`.

## Alternativas consideradas

| Alternativa | Por qué no |
|---|---|
| **Microservice chassis** (librería compartida de runtime) | Acopla ciclos de despliegue y contradice la autonomía (R-AR-5). Además, el repo ya duplicó deliberadamente infraestructura por servicio. |
| **Sólo service template** (copiar un crate) | El template deriva y no verifica el repositorio. Útil como golden path, insuficiente como gobernanza. Se puede añadir después sobre este contrato. |
| **Sólo reglas en Markdown** | No protege nada; es el estado anterior. |
| **Backstage completo** | Sobredimensionado para un monorepo de 16 servicios y sin equipo de plataforma dedicado. El manifiesto conserva los campos relevantes (owner, lifecycle, bounded context, dependencias, SLO) y es compatible conceptualmente. |
| **YAML + schema registry** | Añade dependencia y tooling sin beneficio inmediato; JSON + JSON Schema es suficiente y cero-dependencias. |
| **No congelar deuda y exigir cero desde el día 1** | Bloquearía todos los PRs y mezclaría refactor arquitectónico con features. El baseline hace la deuda visible y la reduce de forma monotónica. |

## Consecuencias

### Positivas

- Un servicio nuevo se declara en un solo archivo y la matriz de CI, el catálogo
  y la documentación se actualizan solos.
- La arquitectura tiene evidencia ejecutable en cada PR, con mensajes
  accionables.
- La deuda es explícita (34 entradas al momento de la decisión) y no puede
  crecer silenciosamente.
- Se elimina la duplicación de listas en CI/project/composes.
- El contrato es legible por humanos y por agentes: `service.json` + reglas +
  catálogo generado.

### Negativas / riesgos

- **Rigidez inicial:** un servicio que necesite un perfil nuevo debe pasar por
  ADR. Es intencional.
- **Mantenimiento del manifiesto:** un cambio de puerto/esquema/evento exige
  actualizarlo en el mismo PR; la suite lo recuerda.
- **Falso sentido de seguridad:** las fitness functions cubren propiedades
  estáticas; no sustituyen tests de integración, caos ni revisión de diseño.
- **Deuda de 34 entradas:** requiere un plan de reducción; se prioriza en
  [`deuda-arquitectura.md`](../../docs/architecture/deuda-arquitectura.md).

### Neutras

- El manifiesto no es Backstage nativo, pero mapea a sus conceptos (Component,
  owner, lifecycle, system/domain, dependsOn, providesApis). Una exportación a
  `catalog-info.yaml` es posible sin cambiar la fuente.

## Criterios de aceptación

- [x] Los 16 servicios tienen `service.json` validado.
- [x] `npm run rules:check` incluye `services-contract` y pasa.
- [x] `npm run architecture:check` pasa y CI lo ejecuta.
- [x] `npm run baseline:guard` incluye la deuda de arquitectura.
- [x] La matriz de CI se deriva de los manifiestos; no quedan listas hardcodeadas.
- [x] El catálogo, la matriz de eventos y la deuda se generan y versionan.
- [x] Las reglas R-MS-9..16 y R-AF-1..5 están definidas y mapeadas.
- [x] La guía de servicio nuevo existe y está enlazada desde el README.

## Seguimiento

1. Reducir la deuda cross-schema (prioridad: escrituras).
2. Añadir `/ready` a los 15 servicios pendientes.
3. Mover las lecturas de entorno restantes a `config.rs`.
4. Evaluar `oasdiff`/Spectral y AsyncAPI para contratos más estrictos.
5. Evaluar un generador `new-service` sobre el manifiesto y el esqueleto.
6. Revisar la cobertura ejecutable (67/254 IDs) y automatizar las reglas
   restantes que sean verificables.
