# D3 — Guion operativo: configurar Dockploy (VPS) — un proyecto, dos entornos

> Documento **operativo** (no normativo): la normativa viva está en
> [`rules/cd/`](./rules/cd/README.md) (`R-CD-9..13`) y el modelo de ramas en
> [`rules/git/00-entornos.md`](./rules/git/00-entornos.md) (`R-GE-*`). Este
> guion es la **lista de tareas** para dejar el VPS acorde a esa normativa.
> Etapa **D3** del modelo. **Revisión de coste (ADR-13):** un solo proyecto con
> dos environments, desarrollo local — no una app por entorno.

## 0. Objetivo (modelo A: 2 entornos + local)

| Etapa | Rama | Dónde corre | Deploy | Riesgo |
|---|---|---|---|---|
| Desarrollo | `develop` (integración) | **local** (`docker compose up` + `npm run demo`) | no se despliega | nulo |
| Stage | `staging` | Dockploy `staging` | auto en push a `staging` | bajo (datos de prueba) |
| Producción | `main` | Dockploy `produccion` | auto en push a `main` | **alto: clientes** |

**Regla de oro:** producción se despliega solo desde `main`, y a `main` solo se
llega por la **promoción** `staging → main` (`R-GE-2/R-GE-4`, `R-CD-10`). El
auto-build actual de Dockploy en cada push a `main` **se mantiene**: el modelo de
ramas es la barrera.

## 1. Requisitos antes de empezar

- Acceso de administrador al VPS (donde corre Dockploy) y a la UI de Dockploy.
- Token/credencial de GitHub que Dockploy usará para clonar **este repo privado**
  (reutilice el que hoy ya usa para el deploy, si existe; no cree uno por entorno).
- Verificar que el repo tiene el `Dockerfile` raíz que Dockploy ya construye.
- Tener a mano `.env.example` → variables por entorno (`DATABASE_URL`,
  `RABBITMQ_URL`, `JWT_SECRET`, `INTERNAL_API_KEY`, `ADMIN_EMAIL/ADMIN_PASSWORD`,
  `COMMISSION_RATE`, `OUTBOX_TABLA`, `*_SERVICE_URL`, `CORS_ORIGINS`, `LOG_LEVEL`).

## 2. Un solo proyecto con dos environments (Dockploy multi-tenancy)

**NO cree tres aplicaciones.** Estructura objetivo (patrón de la doc oficial de
Dockploy, *Core Concepts / Multi-Tenancy*):

```
Proyecto: core-engine
  ├── environment: production        ← services desde rama main
  │     ├── api-gateway (…-production) · services… · postgres-db (db-production)
  └── environment: staging           ← services desde rama staging
        ├── api-gateway (…-staging) · services… · postgres-db (db-staging)
```

Pasos:

1. **Proyecto `core-engine`** (si hoy tiene apps sueltas, móntelas dentro de
   este proyecto o créelo nuevo y migre).
2. **Environments `produccion` y `staging`** en el proyecto. Variables del
   environment: lo que cambia por entorno (credenciales de la BD/vhost, URLs,
   `JWT_SECRET`); lo común va a nivel de proyecto.
3. **Servicios por environment** — para `produccion` desde `main` y para
   `staging` desde `staging` (mismo repositorio, rama distinta). Auto-deploy
   ("redeploy on new commit") en cada environment apuntado **solo** a su rama;
   el de producción **nunca** escucha otra rama.
4. **Build**: mismo `Dockerfile` raíz; sin `ARG`/`ENV` de secretos en la imagen
   (se inyectan en las variables del service/environment — `R-CD-7`).
5. **Variables de entorno**: cada environment recibe las suyas en la pestaña
   *Environment* (Dockploy las escribe al `.env` que consume el compose). La
   plantilla de producción es **`.env.production.example`** (secretos reales y
   únicos); dev usa `.env.example`. **El compose base solo exige
   `POSTGRES_PASSWORD`** (+ las claves de runtime: `DATABASE_URL`, `RABBITMQ_URL`,
   `JWT_SECRET`, `INTERNAL_API_KEY`, …). `GRAFANA_ADMIN_PASSWORD` y
   `RABBITMQ_PASSWORD` solo hacen falta si se activa la capa de observabilidad.
6. **Puertos/red**: gateway expuesto en `8080` por environment (Docker asigna
   puerto/host distinto); red aislada por environment (lo gestiona Dockploy).
7. **Healthcheck**: `GET /health` del gateway de cada environment (para smoke y
   el healtcheck de Dockploy).

> Dockploy corre **`docker-compose.yml`** (base: app + Postgres + RabbitMQ). La
> observabilidad vive en **`docker-compose.observability.yml`** y es **opcional**:
> solo se añade con `-f docker-compose.observability.yml` (y entonces sí exige
> `GRAFANA_ADMIN_PASSWORD` y `RABBITMQ_PASSWORD`).

## 3. Infraestructura compartida (nunca un recurso físico por entorno)

| Recurso | Diseño | Entornos |
|---|---|---|
| Postgres | **1 instancia**, una **base de datos** por entorno | `core_engine_staging` · `core_engine_produccion` |
| RabbitMQ | **1 broker**, un **vhost** por entorno | `/staging` · `/produccion` o por base |
| Prometheus/Grafana | **capa opcional** (`docker-compose.observability.yml`); si se activa, exige `GRAFANA_ADMIN_PASSWORD` | `produccion` (+ opcional staging) |

El esquema se aplica **solo desde el DDL versionado**
(`infra/db/init/01_esquemas.sql` + `99_rls.sql` en Supabase) — `R-DB-6/R-CD-5`.
Migraciones solo hacia adelante.

## 4. Verificaciones por entorno (lista de comprobación)

| # | Comprobación | staging | produccion |
|---|---|---|---|
| 1 | Environment apunta a su rama (`staging` / `main`) | ☐ | ☐ |
| 2 | Auto-deploy solo por push de esa rama | ☐ | ☐ |
| 3 | `GET /health` → 200 con la pila levantada | ☐ | ☐ |
| 4 | Un flujo de lectura real devuelve datos (smoke, `R-CD-12`) | ☐ | ☐ |
| 5 | BD/vhost del entorno aislados (nada compartido) | ☐ | ☐ |
| 6 | Secretos solo en variables de service/environment (`R-CD-7`) | ☐ | ☐ |
| 7 | Rollback posible (redeploy de la versión/commit anterior) | ☐ | ☐ |

> Desarrollo local: `docker compose up -d --build` + `npm run demo` es la
> verificación equivalente antes de cualquier PR (R-GC-5).

## 5. Checklist de seguridad y nombres (al desplegar producción)

- **`CORS_ORIGINS`** con los dominios reales del frontend, **nunca `*`** en producción (R-DS).
- **Rotar `ADMIN_PASSWORD`** tras el primer arranque (identity-service crea el admin en el primer inicio).
- **Secretos únicos por entorno**: `POSTGRES_PASSWORD`, `JWT_SECRET`, `INTERNAL_API_KEY` y `RABBITMQ_PASSWORD` distintos en `staging` y `produccion`; **jamás** los valores de `.env.example` (dev). Plantilla: `.env.production.example`.
- **Credencial de RabbitMQ**: hoy está fija en `infra/rabbitmq/rabbitmq.conf` (deuda, ADR-15). Si la cambias en el `.env`, actualiza también el conf o los servicios no autenticarán contra el broker.
- **Nombre del proyecto/app**: debe ser `core-engine` con environments `staging`/`produccion` (R-CD-9). Si la app arrastra el nombre de otro proyecto (p. ej. `kb-coleccion-...`), renómbrala o recrea el proyecto para no mezclar entornos ajenos.
- **Coexistencia de entornos**: `docker-compose.yml` **no** fija `container_name`, así que `staging` y `produccion` pueden correr a la vez en el mismo VPS sin colisionar.

## 6. Rollback en producción (si algo sale mal)

1. En Dockploy, **redeploy de la versión anterior** de la environment
   `produccion` (commit/digest previo) — `R-CD-13`.
2. **Nunca** revertir migraciones ya aplicadas (`R-CD-5`).
3. Registrar el incidente y corregir por **hotfix** (`hotfix/*` → PR a `main`,
   `R-GE-6`) con **backport** a `staging` y `develop` (`R-GE-5`).

## 7. Configuración "bloqueada" (no tocar)

- El auto-build de producción en push a `main` **es** la entrega (R-CD-10): se
  mantiene; lo que se añade es que ese push solo viene de `staging → main`.
- No creará un environment "desarrollo" siempre-encendido (el desarrollo es
  local, R-CD-9) ni disparadores sueltos de despliegue (webhooks fuera del
  auto-deploy por rama) — `R-CD-6/R-CD-10`.

## 8. Cierre de D3

Con los dos environments operativos y la tabla §4 completa:

1. Hacer un deploy real a `staging` (un push a `staging` o la próxima
   promoción) y registrar su smoke.
2. En la siguiente promoción `staging → main`, verificar el run de `main` verde
   y el smoke de producción (R-PR-8, R-CD-12) antes de cerrar.
3. Actualizar la tabla de comprobación §4 en este doc si algo difiere de la
   práctica real.