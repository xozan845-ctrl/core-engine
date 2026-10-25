# D3 — Guion operativo: configurar Dockploy (VPS) — un proyecto, dos entornos

> Documento **operativo** (no normativo): la normativa viva está en
> [`rules/cd/`](./rules/cd/README.md) (`R-CD-9..13`) y el modelo de ramas en
> [`rules/git/00-entornos.md`](./rules/git/00-entornos.md) (`R-GE-*`). Este
> guion es la **lista de tareas** para dejar el VPS acorde a esa normativa.
> Etapa **D3** del modelo. **Revisión de coste (ADR-13):** un solo proyecto con
> dos environments, desarrollo local — no una app por entorno.

> [!IMPORTANT]
> **Actualización 2026-10-14 — entrega por imágenes (GHCR).** El despliegue ya no
> construye desde el `Dockerfile` raíz en cada push: `release.yml` construye y
> publica **una imagen por servicio** en GHCR al empujar un tag `vX.Y.Z`
> (`R-CD-1..4`), `promote.yml` la mueve a `:stage`/`:prod` **por digest** (`R-CD-3`)
> y el entorno arranca con **`docker-compose.stage.yml`** (stage) / **`docker-compose.yml`**
> (producción), que consumen esas imágenes. Pasos nuevos: `docker login ghcr.io` en el host (o
> una credencial de registry en Dockploy) y las variables del entorno en la
> pestaña *Environment* (plantillas `.env.stage.example` /
> `.env.production.example`): Dockploy las escribe a `.env`, el archivo que el
> compose consume por defecto (R-CD-7). Runbook:
> [`runbook-entrega.md`](./runbook-entrega.md).

## 0. Objetivo (modelo A: 2 entornos + local)

| Etapa | Rama | Dónde corre | Deploy | Riesgo |
|---|---|---|---|---|
| Desarrollo | `develop` (integración) | **local** (`docker compose -f docker-compose.dev.yml up` + `npm run demo`) | no se despliega | nulo |
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
- **Credencial de registry para GHCR** (PAT con `read:packages`), o `docker login
  ghcr.io` en el VPS: las imágenes de la API son **privadas** y el entorno las
  consume promovidas (ver nota de arriba). Sin esto, el `pull` falla.
- Verificar que el repo tiene los composes por entorno (`docker-compose.yml` de
  producción, `docker-compose.stage.yml`) que consumen las imágenes de GHCR.
- Tener a mano `.env.stage.example` / `.env.production.example` → variables por
  entorno (`DATABASE_URL`, `RABBITMQ_URL`, `JWT_SECRET`, `INTERNAL_API_KEY`,
  `ADMIN_EMAIL/ADMIN_PASSWORD`, `COMMISSION_RATE`, `OUTBOX_TABLA`, `*_SERVICE_URL`,
  `CORS_ORIGINS`, `LOG_LEVEL`). En Dockploy van a la pestaña *Environment* del
  environment; el compose las lee del `.env` que Dockploy escribe.

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
4. **Imágenes (GHCR)**: cada environment consume la imagen **promovida**
   (`:staging`/`:prod`) desde `docker-compose.yml` (prod)/`docker-compose.stage.yml`,
   sin construir en el host (`R-CD-3`); sin `ARG`/`ENV` de secretos en la imagen
   (se inyectan en las variables del service/environment — `R-CD-7`).
5. **Variables de entorno**: cada environment recibe las suyas en la pestaña
   *Environment* (Dockploy las escribe al `.env` que consume el compose). La
   plantilla de producción es **`.env.production.example`** (secretos reales y
   únicos); dev usa `.env.example`. **El compose de producción solo exige
   `POSTGRES_PASSWORD`** (+ las claves de runtime: `DATABASE_URL`, `RABBITMQ_URL`,
   `JWT_SECRET`, `INTERNAL_API_KEY`, …). `GRAFANA_ADMIN_PASSWORD` y
   `RABBITMQ_PASSWORD` solo hacen falta si se activa la capa de observabilidad.
6. **Puertos/red**: el gateway se publica **solo en loopback**
   (`127.0.0.1:8080:8080`); el proxy de borde (Traefik/Dockploy) enruta el 443
   hacia él. Nunca publicar `8080:8080` en todas las interfaces: saltaría TLS y
   el propio proxy. Red aislada por environment (lo gestiona Dockploy).
7. **Healthcheck/Readiness**: `GET /health` (liveness del gateway; lo usa el
   healthcheck de Dockploy, siempre abierto) y `GET /ready` (readiness agregada:
   responde **503** si algún microservicio de producción no está `ok`; lo usa el
   smoke, R-CD-12). `/ready` exige `x-internal-key` salvo `READY_PUBLIC=true`
   (solo desarrollo) y está limitado a 10 req/min por IP con caché de 5 s.

> Dockploy corre el compose del entorno: **`docker-compose.yml`** (producción) o
> **`docker-compose.stage.yml`** (imágenes de GHCR + Postgres + RabbitMQ). La
> observabilidad vive en **`docker-compose.observability.yml`** y es **opcional**:
> solo se añade con `-f docker-compose.observability.yml` (y entonces sí exige
> `GRAFANA_ADMIN_PASSWORD`).

## 3. Infraestructura compartida (nunca un recurso físico por entorno)

| Recurso | Diseño | Entornos |
|---|---|---|
| Postgres | **1 instancia**, una **base de datos** por entorno | `core_engine_staging` · `core_engine_produccion` |
| RabbitMQ | **1 broker**, un **vhost** por entorno | `/staging` · `/produccion` o por base |
| MinIO (ADR-24) | Bucket **por tenant** (`t-<tenantId>`) dentro del MinIO del entorno; el `9000` queda **solo en loopback** (lo alcanza Traefik) y el acceso público va por su dominio HTTPS (`MINIO_PUBLIC_ENDPOINT`) | `staging` · `produccion` |
| Prometheus/Grafana | **capa opcional** (`docker-compose.observability.yml`); si se activa, exige `GRAFANA_ADMIN_PASSWORD` | `produccion` (+ opcional staging) |

El esquema se aplica **solo desde el DDL versionado**
(`infra/db/init/01_esquemas.sql` + `99_rls.sql` en Supabase) — `R-DB-6/R-CD-5`.
Migraciones solo hacia adelante.
> **Entornos ya desplegados**: un DDL nuevo (p. ej. `09_storage.sql`, ADR-24) **no**
> se aplica solo (el init corre únicamente en un volumen Postgres nuevo). Aplícalo
> a mano antes de desplegar el servicio que lo usa:
> `docker compose exec -T postgres psql -U $POSTGRES_USER -d $POSTGRES_DB < infra/db/init/09_storage.sql`.

## 4. Verificaciones por entorno (lista de comprobación)

| # | Comprobación | staging | produccion |
|---|---|---|---|
| 1 | Environment apunta a su rama (`staging` / `main`) | ☐ | ☐ |
| 2 | Auto-deploy solo por push de esa rama | ☐ | ☐ |
| 3 | `GET /ready` con `x-internal-key: $INTERNAL_API_KEY` → 200 (todos los servicios `ok`) | ☐ | ☐ |
| 4 | Un flujo de lectura real devuelve datos (smoke, `R-CD-12`) | ☐ | ☐ |
| 5 | BD/vhost del entorno aislados (nada compartido) | ☐ | ☐ |
| 6 | Secretos solo en variables de service/environment (`R-CD-7`) | ☐ | ☐ |
| 7 | Rollback posible (redeploy de la versión/commit anterior) | ☐ | ☐ |
| 8 | TLS en **todos** los dominios (API y MinIO) | ☐ | ☐ |
| 9 | `/docs`, `/docs-json` y `/metrics` ocultos (`DIAGNOSTICOS_PUBLICOS=false`) | ☐ | ☐ |
| 10 | MinIO sin puerto público (`9000` solo detrás de Traefik con TLS) | ☐ | ☐ |
| 11 | Gate de staging (`EDGE_BASIC_AUTH`) — obligatorio (`REQUIRE_EDGE_AUTH=true`; el arranque falla sin él); producción sin gate | ☐ | n/a |
| 12 | `EDGE_BASIC_BYPASS_ORIGINS` solo con el origen público de Firebase (demo); el resto de orígenes sigue con Basic | ☐ | n/a |
| 13 | `CORS_ORIGINS` con los orígenes reales del frontend | ☐ | ☐ |
| 14 | `TRUST_PROXY=1` y rate limit por IP real (probar dos IPs o revisar `x-ratelimit-remaining`) | ☐ | ☐ |
| 15 | `8080` solo en loopback y cerrado en el firewall del VPS | ☐ | ☐ |

> `EDGE_BASIC_AUTH` debe existir también como secreto del GitHub Environment
> (`EDGE_BASIC_AUTH`) para que el smoke remoto se autentique en staging. Sin
> él, el paso `npm run smoke:remote` recibe 401 en el login.
>
> `EDGE_BASIC_BYPASS_ORIGINS` es una excepción de **demo público**, no una
> autenticación: `Origin` puede falsificarse fuera del navegador. Solo debe
> contener el origen exacto de Firebase
> (`https://kb-coleccion-internacional-ni.web.app`) y nunca usarse en
> producción. El resto de orígenes sigue exigiendo Basic.
>
> Desarrollo local: `docker compose -f docker-compose.dev.yml up -d --build` + `npm run demo` es la
> verificación equivalente antes de cualquier PR (R-GC-5).

## 5. Checklist de seguridad y nombres (al desplegar producción)

- **`CORS_ORIGINS`** con los dominios reales del frontend, **nunca `*`** en producción (R-DS).
- **Rotar `ADMIN_PASSWORD`** tras el primer arranque (identity-service crea el admin en el primer inicio).
- **Secretos únicos por entorno**: `POSTGRES_PASSWORD`, `JWT_SECRET`, `INTERNAL_API_KEY` y `RABBITMQ_PASSWORD` distintos en `staging` y `produccion`; **jamás** los valores de `.env.example` (dev). Plantilla: `.env.production.example`.
- **Credencial de RabbitMQ**: ya **no** está en el repo; se inyecta por entorno desde `RABBITMQ_USER`/`RABBITMQ_PASSWORD` (ADR-21). Debe ser **única por entorno**. En un volumen ya inicializado, cambiar la variable **no** re-crea el usuario: rota la credencial por la gestión de RabbitMQ o recrea el volumen.
- **Nombre del proyecto/app**: debe ser `core-engine` con environments `staging`/`produccion` (R-CD-9). Si la app arrastra el nombre de otro proyecto (p. ej. `kb-coleccion-...`), renómbrala o recrea el proyecto para no mezclar entornos ajenos.
- **Coexistencia de entornos**: `docker-compose.yml` **no** fija `container_name`, así que `staging` y `produccion` pueden correr a la vez en el mismo VPS sin colisionar.
- **Puertos**: en producción/staging **solo se publica el `gateway` (8080)**; Postgres y RabbitMQ son **internos**. **No** uses `docker-compose.dev.yml` en producción (ese entorno abre `5432`/`5672`/`15672` y construye local).
- **Servicios fuera de producción**: `commissions-service` y `market-intelligence-service` **no se despliegan por ahora** (no están en `docker-compose.yml`; viven en `docker-compose.dev.yml`). El `up --remove-orphans` de producción **elimina** sus contenedores si venían de un despliegue anterior.

## 6. Rollback en producción (si algo sale mal)

1. En Dockploy, **redeploy de la versión anterior** de la environment
   `produccion` (commit/digest previo) — `R-CD-13`.
2. **Nunca** revertir migraciones ya aplicadas (`R-CD-5`).
3. Registrar el incidente y corregir por **hotfix** (`hotfix/*` → PR a `main`,
   `R-GE-6`) con **backport** a `staging` y `develop` (`R-GE-5`).

## 7. Configuración "bloqueada" (no tocar)

- La entrega de producción son las **imágenes promovidas** (`:prod`) que consume
  `docker-compose.yml` (R-CD-3/R-CD-10). El gate de producción se mantiene:
  a `main` solo se llega por `staging → main`, y solo se promueve un tag cuyo
  commit tiene `ci.yml` verde (R-CD-2).
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

## 9. Errores frecuentes en el deploy (Dockploy)

- **`required variable POSTGRES_PASSWORD is missing a value`** — falla en la
  *interpolación* del compose, antes de construir. El environment no tiene
  `POSTGRES_PASSWORD`. Dockploy escribe la pestaña *Environment* al `.env` que
  consume el compose: añade ahí las variables de `.env.production.example`
  (mínimo `POSTGRES_PASSWORD`; y para que arranquen los servicios:
  `DATABASE_URL`, `RABBITMQ_URL`, `JWT_SECRET`, `INTERNAL_API_KEY`,
  `ADMIN_EMAIL`, `ADMIN_PASSWORD`, `CORS_ORIGINS`). Valor fuerte y **único por
  entorno** (R-CD-7). No hay default a propósito: mejor fallar al inicio que
  arrancar con una contraseña débil.
- **`env file ./.env.stage not found`** — el compose exigía un archivo que
  Dockploy **no** escribe: su pestaña *Environment* va a `.env`, y además
  Dockploy **re-clona** el repo en cada deploy, así que un archivo creado a mano
  en el VPS se pierde. Los composes de stage/producción leen `ENV_FILE`, con
  `.env` por defecto (R-CD-7): en el VPS no dependas de `.env.stage`/`.env.production`.
- **El proyecto/app no se llama `core-engine`** (p. ej.
  `kb-coleccion-backend-corporativo-pegot2`): renómbralo/recrea el proyecto
  (R-CD-9). Compose nombra los contenedores como `<proyecto>-<servicio>-1`, así
  que un nombre ajeno no rompe el arranque, pero mezcla dominios/entornos.
- **`unauthorized`/`denied` al hacer pull de `ghcr.io/...`**: el host o Dockploy no
  está autenticado contra GHCR. Configura una credencial de registry
  (`read:packages`) y `docker login ghcr.io`; además, la etiqueta `:staging`/`:prod`
  debe existir (alguien debe haber corrido `promote.yml` con un `source` publicado).
- **`commissions-service`/`market-intelligence-service` siguen corriendo** tras
  un deploy: no están en `docker-compose.yml` (producción); el `up --remove-orphans` los elimina.
  Si persistieran:
  `docker rm -f <proyecto>-commissions-service-1 <proyecto>-market-intelligence-service-1`
  (ADR-17).