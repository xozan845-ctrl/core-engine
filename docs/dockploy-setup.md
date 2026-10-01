# D3 — Guion operativo: configurar Dockploy (VPS) para los tres entornos

> Documento **operativo** (no normativo): la normativa viva está en
> [`rules/cd/`](./rules/cd/README.md) (`R-CD-9..13`) y el modelo de ramas en
> [`rules/git/00-entornos.md`](./rules/git/00-entornos.md) (`R-GE-*`). Este
> guion es la **lista de tareas** para dejar el VPS acorde a esa normativa.
> Etapa **D3** del modelo (D1 reglas ✅ · D2 ramas + protección ✅ · **D3 este guion** · D4 validación).

## 0. Objetivo

| Entorno | Rama (protegida) | App Dockploy | Deploy | Riesgo |
|---|---|---|---|---|
| Desarrollo | `develop` | `core-engine-desarrollo` | auto en push a `develop` | nulo (solo devs) |
| Stage | `staging` | `core-engine-stage` | auto en push a `staging` | bajo (datos de prueba) |
| Producción | `main` | `core-engine-produccion` | auto en push a `main` | **alto: clientes** |

**Regla de oro:** producción se despliega solo desde `main`, y a `main` solo se
llega por la **promoción** `staging → main` (`R-GE-2/R-GE-4`, `R-CD-10`). El
auto-build actual de Dockploy en cada push a `main` **se mantiene**: el modelo de
ramas es la barrera. Lo que este guion busca es que existan **tres apps
separadas**, cada una anclada a SU rama, y que nada más pueda disparar un deploy
de producción.

## 1. Requisitos antes de empezar

- Acceso de administrador al VPS (donde corre Dockploy) y a la UI de Dockploy.
- Token/credencial de GitHub que Dockploy usará para clonar **este repo privado**
  (si Dockploy ya lo usa hoy para el deploy a producción, reutilícelo — no cree
  un token nuevo por app si no hace falta).
- Verificar que el repo tiene el `Dockerfile` raíz que Dockploy ya construye.
- Tener a mano `.env.example` → `.env` de cada entorno (Secret: `DATABASE_URL`,
  `RABBITMQ_URL`, `JWT_SECRET`, `INTERNAL_API_KEY`, `ADMIN_EMAIL/ADMIN_PASSWORD`,
  `COMMISSION_RATE`, `OUTBOX_TABLA`, `*_SERVICE_URL`, `CORS_ORIGINS`, `LOG_LEVEL`).

## 2. Crear las apps (una por entorno)

Para **cada** app (`core-engine-desarrollo`, `core-engine-stage`,
`core-engine-produccion`):

1. **Nueva app** en Dockploy con el nombre exacto de la tabla del punto 0.
2. **Repositorio**: apuntar a `xozan845-ctrl/core-engine`.
3. **Rama de despliegue**:
   - desarrollo → `develop`
   - stage → `staging`
   - produccion → `main`
4. **Auto-deploy**: activado *"redeploy on new commit"* (push a esa rama). Asegurar
   que el de produccion **solo** escucha `main` (nada de "todas las ramas").
5. **Build**: mismo `Dockerfile` raíz; sin `ARG`/`ENV` de secretos en la imagen
   (injectarlos en runtime de la app — `R-CD-7`).
6. **Variables de entorno** de la app: **cada entorno con sus propias**:
   - `DATABASE_URL` del Postgres **de ese entorno** (ver §4). Nunca la de otro
     entorno, y jamás la de producción en stage/desarrollo.
   - `RABBITMQ_URL`, `JWT_SECRET` (distinto por entorno si se puede), el resto
     del `.env`.
7. **Puertos / red**: el gateway debe quedar expuesto en `8080` (o el que use el
   proxy del VPS); los servicios internos en su red interna de Dockploy.
8. **Healthcheck** (para smoke y para que Dockploy marque la app sana):
   `GET /health` en el gateway del entorno.

## 3. Verificaciones por entorno (lista de comprobación)

Al terminar cada app deje constancia (marcar ✅ en el PR de validación / este doc):

| # | Comprobación | desarrollo | stage | produccion |
|---|---|---|---|---|
| 1 | App apunta a `develop` / `staging` / `main` | ☐ | ☐ | ☐ |
| 2 | Auto-deploy solo por push de esa rama | ☐ | ☐ | ☐ |
| 3 | `GET /health` → 200 con la app levantada | ☐ | ☐ | ☐ |
| 4 | Un flujo de lectura real devuelve datos (smoke, `R-CD-12`) | ☐ | ☐ | ☐ |
| 5 | BD del entorno aislada (nada compartido) | ☐ | ☐ | ☐ |
| 6 | Secretos solo en variables de la app (`R-CD-7`) | ☐ | ☐ | ☐ |
| 7 | Rollback posible (redeploy de la versión/commit anterior) | ☐ | ☐ | ☐ |

> El smoke de **producción** no se hace "de prueba": se corre al promover la
> primera vez y queda anotado en el PR de promoción (`R-GE-3/R-CD-12`).

## 4. Bases de datos por entorno

- Cada entorno usa **su propia** BD (Postgres en el VPS o Supabase por entorno).
  Prohibido compartir BD entre entornos (`R-CD-9`).
- El esquema se aplica **solo desde el DDL versionado**
  (`infra/db/init/01_esquemas.sql` + `99_rls.sql` en Supabase) — `R-DB-6/R-CD-5`.
  Nada de mutaciones ad-hoc: un cambio de esquema es un cambio de código que
  entra por el pipeline (y migraciones **solo hacia adelante**).
- `desarrollo`/`stage`: datos de prueba. `produccion`: datos reales.

## 5. Rollback en producción (si algo sale mal)

1. En Dockploy, **redeploy de la versión anterior** de `core-engine-produccion`
   (digest/commit previo) — `R-CD-13`.
2. **Nunca** revertir migraciones ya aplicadas (`R-CD-5`).
3. Registrar el incidente y corregir por **hotfix** (`hotfix/*` → PR a `main`,
   `R-GE-6`) con **backport** a `staging` y `develop` (`R-GE-5`).

## 6. Configuración "bloqueada" (no tocar)

- El auto-build de producción en push a `main` **es** la entrega (R-CD-10): se
  mantiene tal cual. Lo que se añade es que ese push solo viene de `staging → main`.
- No configurar otros disparadores de despliegue (webhooks sueltos, deploys desde
  ramas de feature) — `R-CD-6/R-CD-10`.

## 7. Cierre de D3

Cuando las tres apps estén operativas y la tabla §3 esté completa:

1. Abrir un PR de promoción `staging → main` **solo documental** (o la primera
   promoción real que toque) informando "D3 aplicado".
2. Verificar el run de `main` verde y el deploy con smoke (`R-PR-8`, `R-CD-12`).
3. Quedará la **deuda de entornos desactivada**: esta etapa elimina el "no había
   a dónde desplegar" de la normativa antigua (ya corregido en `cd/`).