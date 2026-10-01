# 00 — Entornos y promoción (desarrollo · stage · producción)

Modelo de ramas que sostiene el despliegue de [`../cd/`](../cd/README.md): de
dónde nacen las ramas, hacia dónde apuntan los PR y qué significa cada merge.
**Tres etapas, tres ramas protegidas** — desarrollo local en `develop` y dos
etapas desplegadas en `staging`/`main` (branch protection en GitHub: sin push
directo, sin fuerza, solo PR con checks en verde — R-PR-6, R-CD-9):

| Etapa | Rama | Dónde corre | Uso |
|---|---|---|---|
| Desarrollo | `develop` (integración) | **local** (`docker compose up`) | integración diaria de features y puerta de entrada del trabajo; sin despliegue (R-CD-9) |
| Stage | `staging` | Dockploy, environment `staging` | validación de aceptación antes de tocar producción (smoke, R-CD-12) |
| Producción | `main` | Dockploy, environment `produccion` | lo que despliega Dockploy a los clientes; solo recibe promociones |

| ID | Regla |
|---|---|
| R-GE-1 | **Etapas en ramas protegidas**: `develop` es la rama de **integración** (puerta de entrada de todo el trabajo) y `staging`/`main` son las ramas de las etapas **desplegadas** (stage y producción — R-CD-9). El desarrollo corre **local** y no se despliega. Cada una está protegida: no se pushea directo, no se fuerza (R-GP-3) y todo llega por PR con checks verdes en la cabeza (R-PR-6) y run de la rama destino verificado tras el merge (R-PR-8). |
| R-GE-2 | **Promoción en un solo sentido**: las features nacen de `develop` y su PR apunta a `develop`; el paso a stage es un PR `develop → staging`; el paso a producción es un PR `staging → main` (promoción deliberada, R-CD-10). Prohibido saltarse un salto (un feature directo a `main`) y prohibido invertir el sentido de una promoción. |
| R-GE-3 | **Cada promoción entrega evidencia**: el PR de promoción (`develop → staging`, `staging → main`) incluye en su cuerpo el estado de las verificaciones del salto (tests en verde del repo y smoke del entorno origen — R-CD-12) y, tras el merge, se verifica el run de `ci.yml` en la rama destino (R-PR-8, R-CD-11). Un salto sin evidencia no se fusiona. |
| R-GE-4 | **`main` solo avanza por promoción**: nada se edita a mano en `main`; el único camino es el merge `staging → main` como decisión deliberada (R-GE-2). Un cambio que tumba producción se atiende con hotfix (R-GE-6) o rollback (R-CD-13), nunca con un push directo ni con un merge no promovido. |
| R-GE-5 | **Backport obligatorio tras un hotfix**: todo cambio que entra a `main` por hotfix (R-GE-6) se re-aplica hacia `staging` y `develop` con PRs de vuelta, para que la siguiente promoción no revierta la corrección. Prohibido dejar `main` con cambios que `develop`/`staging` no tengan. |
| R-GE-6 | **Hotfix de incidente**: un incidente en producción se corrige con una rama `hotfix/...` creada desde `main`, su PR a `main` con el fix y la evidencia en el cuerpo (R-GP-3: sin force, commit nuevo); tras el merge se verifica el run de `main` (R-PR-8), se ejecuta el smoke de producción (R-CD-12) y se aplica el backport (R-GE-5). |
| R-GE-7 | **Promoción sin historiales duplicados**: si un PR de promoción reporta conflicto porque el contenido ya existe en la rama destino por otro camino (mismos cambios con SHAs distintos, p. ej. por un viaje previo por el pipeline), **no** se resuelve integrando historiales duplicados ni con force (R-GP-3): se cierra el PR con la razón (R-PR-9) y se recrea la promoción **desde la rama destino** (rama nueva sobre la destino + cherry-pick del delta de commits deseados), verificando que el árbol queda **idéntico** a la rama origen (`git diff --quiet <rama-origen>` = vacío) antes de reabrir el PR. Regla de oro: el contenido cuenta, no la procedencia de los SHAs. |

> Este archivo fija el **modelo de ramas**; el despliegue físico en Dockploy
> (proyecto con environments, gate de producción, rollback) vive en
> [`../cd/02-entornos.md`](../cd/02-entornos.md) (`R-CD-9..13`), que consume
> estas ramas.