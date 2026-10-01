# 02 — Etapas y despliegue (Dockploy: un proyecto, dos entornos) (`R-CD-9..13`)

Reglas del despliegue **que sí existe hoy**: el VPS con Dockploy construye desde
el `Dockerfile` raíz y despliega por rama. Consumen el modelo de ramas de
[`../git/00-entornos.md`](../git/00-entornos.md) (`R-GE-*`).

> **Enmienda 2026-10-01 (revisión de coste, ADR-13):** antes se preveía "una app
> Dockploy por entorno" (3 apps). Se descartó por coste y por práctica de CD: el
> desarrollo corre **local** y Dockploy aloja **un solo proyecto con dos
> environments** (`staging`, `produccion`).

## Mapa de etapas → despliegue

| Etapa | Rama | Dónde corre | Deploy |
|---|---|---|---|
| Desarrollo | `develop` (integración) | **local** (`docker compose up` + `npm run demo`) | no se despliega |
| Stage | `staging` | Dockploy, environment `staging` | auto en push a `staging` |
| Producción | `main` | Dockploy, environment `produccion` | auto en push a `main` (solo por promoción) |

| ID | Regla |
|---|---|
| R-CD-9 | **Un proyecto Dockploy con environments por etapa**: la entrega vive en un **solo proyecto** `core-engine` con dos environments (multi-tenancy nativo de Dockploy): `staging` (rama `staging`) y `produccion` (rama `main`) — mismos servicios, variables **propias por environment** y **red aislada**. El entorno de **desarrollo corre local** (`docker compose up` del repo) y **nunca se despliega** a Dockploy. La infraestructura se comparte con **aislamiento lógico**: un solo Postgres con **una base de datos por entorno** y un solo RabbitMQ con **un vhost por entorno** — jamás un recurso físico por entorno (práctica de coste; ver ADR-13). Los previews por feature de Dokploy son opcionales y solo se habilitan con un caso de uso documentado. |
| R-CD-10 | **Producción es promoción, no publicación**: Dockploy despliega producción con cada push a `main`, y eso se mantiene **porque a `main` solo se llega por el merge `staging → main`** (R-GE-2/R-GE-4). Prohibido configurar un despliegue de producción desde cualquier otra rama o desde un webhook no controlado, y prohibido un despliegue siempre-encendido de desarrollo (el desarrollo es local, R-CD-9). |
| R-CD-11 | **Gate de producción: el merge promovido trae CI verde**: antes de dar por buena una promoción a `main` se exige el run de `ci.yml` en `success` en el SHA de la promoción (4 jobs; R-PR-8, R-CI-6, R-CD-2). Como Dockploy construye en paralelo, el gate no bloquea su build: si el run de `main` sale en rojo tras el merge, es **incidente** — rollback (R-CD-13) o hotfix (R-GE-6), nunca "se queda así". |
| R-CD-12 | **Smoke tras cada despliegue**: tras cada deploy a `staging` y a `produccion` se ejecuta el smoke del entorno: `GET /health` del gateway responde `200` y un flujo de lectura real devuelve datos (ej. un TC de lectura de `npm run demo` contra el entorno o `GET /api/v1/catalog/productos` autenticado). En desarrollo local la verificación equivalente es `npm run demo`. El resultado se anota en el PR de promoción (R-GE-3). Sin smoke en verde no se promueve al siguiente entorno. |
| R-CD-13 | **Rollback = redesplegar el estado anterior**: un despliegue malo se revierte en Dockploy redesplegando la versión/imagen anterior de la environment del entorno. Nunca se revierte el historial de migraciones (R-CD-5): deshacer SQL ya aplicado en producción significa perder datos. Tras el rollback se corrige con hotfix (R-GE-6) y backport (R-GE-5). |