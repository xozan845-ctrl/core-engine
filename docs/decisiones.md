# Decisiones de Arquitectura (ADR) — Core Engine Core Engine

Este archivo registra decisiones técnicas deliberadas que se apartan del texto literal del documento de referencia (Core Engine_Informe.pdf) o que requieren justificación para auditoría. Cada decisión se enmarca como **ADR-XX** con estado, contexto y consecuencias.

---

## ADR-01: Montos en centavos (`*_cents`) — desviación de nombres de contrato

- **Fecha**: 2026-08-09
- **Estado**: Aceptado
- **Contexto**: El documento (Tabla 22, 5.7) especifica campos como `total`, `monto` en los payloads de eventos y respuestas. La implementación usa `total_cents`, `monto_cents`, `precio_base_cents`, etc.
- **Decisión**: Mantener la convención `*_cents` (enteros) en todo el backend.
- **Justificación**:
  - Control OWASP A02: evitar aritmética de punto flotante en montos monetarios.
  - `Money` class encapsula la serialización `"1150.00"` (dos decimales) en `string()`.
  - Cambiar a nombres sin `_cents` rompería la seguridad de tipos y requeriría conversiones en cada límite de servicio.
- **Mitigación**: La API expone montos formateados en respuestas HTTP; los contratos de eventos internos usan centavos. En la documentación OpenAPI/Swagger los campos aparecen con `format: "currency"` y ejemplos en C$ con dos decimales.

---

## ADR-02: Límite de margen RN-01 (0–90 %)

- **Fecha**: 2026-08-09
- **Estado**: Corregido (era 0–100 %)
- **Contexto**: `Money.aplicarMargen` aceptaba 100 % en `shared/src/money.ts:58`, pero RN-01 y `MARGEN_MAXIMO = 90` en `constants.ts` exigen 90 %.
- **Acción**: Cambiado el límite a 90 y mensaje de error acorde. Test unitario actualizado.
- **Impacto**: Ninguno en producción (los servicios validaban contra `MARGEN_MAXIMO`); el helper compartido ahora es consistente.

---

## ADR-03: Rol "logística" — no existe en el MVP

- **Fecha**: 2026-08-09
- **Estado**: Documentado
- **Contexto**: El documento (Tabla 21, 4.3) menciona rol "logística" para avanzar estado de orden (`PATCH /api/v1/orders/:id/estado`). En la implementación los roles son `admin`, `vendedor`, `comprador` (Tabla 15 RLS).
- **Decisión**: Las rutas de logística exigen `admin`. El servicio `logistics-service` existe y consume eventos (`payment.procesado`, `order.status.updated`), pero no hay un rol JWT separado.
- **Justificación**: Simplifica RBAC y RLS en el MVP; el admin cubre la operación de bodega. Si se requiere separación, se añade el rol en `ROLES` y migraciones RLS sin romper APIs.

---

## ADR-04: Frontend Angular + Capacitor — pendiente

- **Fecha**: 2026-08-09
- **Estado**: Pendiente (entregable separado)
- **Contexto**: El documento (AD-07, Cap. 3.3, Tabla 30) exige SPA Angular servida en web y envuelta con Capacitor para Android/iOS.
- **Decisión**: No incluido en este repositorio (backend only). El frontend se desarrollará en repositorio aparte o carpeta `frontend/` futura.
- **Impacto**: RN-05 (carrito 30 min) se implementa en backend (orders-service) para permitir API-first; el frontend consumirá `/api/v1/carrito`.

---

## ADR-05: Rate limiting real en Gateway (429 estructurado)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: El documento (5.4, 5.7) exige limitación de tasa en el gateway y error 429 con formato `{codigo, mensaje, detalles}`. `ThrottlerGuard` de Nest estaba declarado pero inerte (gateway sin controllers).
- **Decisión**: `RateLimitMiddleware` en memoria (ventana fija por IP): global 300 req/min, login 10 req/min. Respuesta 429 con `codigo: 'DEMASIADAS_PETICIONES'`, cabeceras `x-ratelimit-*` y `Retry-After`. `ThrottlerModule` removido.
- **Consecuencia**: El 429 ahora es real y estructurado.

---

## ADR-06: Normalización total de errores HTTP (doc 5.7)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `DomainErrorFilter` dejaba pasar `HttpException` de Nest tal cual (sin `{codigo, mensaje, detalles}`).
- **Decisión**: Todo error (DomainError, HttpException, 500) sale normalizado. Mapeo status→codigo: 400 `SOLICITUD_INVALIDA`, 401 `NO_AUTORIZADO`, 403 `ACCESO_DENEGADO`, 404 `NO_ENCONTRADO`, 409 `CONFLICTO`, 429 `DEMASIADAS_PETICIONES`, else `HTTP_<status>`.

---

## ADR-07: Trazabilidad y métricas en TODOS los microservicios

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `TrazabilidadInterceptor` solo estaba en gateway (como `APP_INTERCEPTOR`); los microservicios no registraban `http_peticion_*` ni propagaban `request_id` a logs.
- **Decisión**:
  - `TrazabilidadInterceptor` ahora envuelve el handler en `ejecutarConContexto` → `request_id` disponible en `Logger` vía `AsyncLocalStorage`.
  - Cada `main.ts` registra `app.useGlobalInterceptors(new TrazabilidadInterceptor(metrics, NOMBRE_SERVICIOS.X))`.
  - Gateway: `PasarelaMiddleware` genera `x-request-id` y registra métricas en `finish`.

---

## ADR-08: Reintento DLQ vivo con backoff exponencial (AD-04 / TC-06)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `RabbitService.reintentarDesdeDlq` existía pero nunca se invocaba (código muerto). El documento exige backoff y reinyección automática.
- **Decisión**:
  - `RabbitService` rastrea colas declaradas y expone `activarReintento(intervaloMs=10s)` con poller `setInterval`.
  - Al dead-letter se guarda `x-routing-key-original` para reinyectar al exchange de eventos con la routing key correcta.
  - Todos los consumidores (catalog, orders, logistics, commissions, finance, stores) llaman `rabbit.activarReintento()` en `onModuleInit`.

---

## ADR-09: RN-02 — Oferta "agotada" al llegar stock 0 (consumidor real)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `OfertasService.sincronizarStockDeOferta` existía pero no se invocaba (código muerto). No había consumidor de `stock.updated`.
- **Decisión**:
  - `catalog-service` al reservar/reintegrar stock publica `stock.updated` con `{items: [{sku, stock_restante}]}`.
  - `stores-service` nuevo `StockConsumer` consume `stock.updated` y llama `sincronizarStockDeOferta(sku, stock_restante)` con idempotencia (`stores.eventos_procesados`).
  - Cola `stores.stock` añadida en `rabbit.constants.ts`.

---

## ADR-10: RN-05 — Carrito con expiración 30 min (backend)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: El documento trata el carrito como concepto de frontend (SPA) sin endpoints ni tabla. La auditoría exigía entidad + endpoints + expiración.
- **Decisión**: `orders.carritos` (PK `comprador_id`, `items_json`, `total_cents`, `actualizado_en`) con purga perezosa `actualizado_en > NOW() - 30 min`. Endpoints `/api/v1/carrito` (GET, POST items, PATCH item, DELETE item, DELETE all) bajo rol `comprador`. Checkout opcional con `usar_carrito: true` vacía el carrito en la misma transacción que crea la orden.
- **RLS**: Comprador solo su carrito; admin acceso total.

---

## ADR-11: Migraciones SQL y RLS fuera de Docker (doc AD-08)

- **Fecha**: 2026-08-09
- **Estado**: Confirmado
- **Contexto**: `infra/db/init/01_esquemas.sql` se monta en `docker-entrypoint-initdb.d` (Docker local). `infra/db/supabase/99_rls.sql` se aplica manualmente en staging/prod (Supabase provee `auth.uid()/auth.rol()`).
- **Acción**: Añadidas tablas `orders.carritos` e `stores.eventos_procesados` en `01_esquemas.sql` + políticas RLS en `99_rls.sql`.

---

## ADR-12: Entrega continua con tres entornos (desarrollo → stage → producción)

- **Fecha**: 2026-10-01
- **Estado**: Aceptado
- **Contexto**: el VPS (Dockploy) redispliega **producción en cada push a `main`**,
  de modo que cualquier commit que pase los tests aterriza en producción sin
  pasar por un entorno previo. `main` no tenía branch protection (HTTP 404) y
  las reglas de despliegue (`R-CD` en `ci/03-deploy.md`) asumían "no hay a dónde
  desplegar" — ambas cosas falsas frente a la realidad.
- **Decisión**: modelo de **tres entornos por rama protegida** — `develop`
  (desarrollo) → `staging` (stage) → `main` (producción), con promoción en un
  solo sentido (PRs `develop → staging` y `staging → main`). A `main` solo llega
  un merge de promoción con CI verde y smoke (R-GE-2/3, R-CD-11/12); el
  auto-deploy de Dockploy en `main` se mantiene porque la promoción es la
  barrera. Cambios normativos: nueva área `docs/rules/cd/` (`R-CD-1..8`
  trasladadas de `ci/03` sin renumerar + `R-CD-9..13` nuevas de Dockploy),
  `docs/rules/git/00-entornos.md` (`R-GE-1..6`), enmienda `R-CI-1` con el
  gatillo de `staging` en `ci.yml`, y actualización de `R-PR-1`/`R-PR-8`/`R-GP-3`.
- **Consecuencias**: se crearon las ramas `develop` y `staging` con branch
  protection en las tres ramas de entorno: **PR obligatorio sin aprobación**
  (`required_approving_review_count: 0` — en repo unipersonal GitHub no cuenta la
  aprobación del propio autor y bloquearía cada merge; la revisión sigue siendo
  el cuerpo del PR con evidencia, R-PR-4), **4 checks de CI** (Security Gate,
  ESLint, Unit Tests + Coverage, Build), **historial lineal** y sin push directo
  ni force (ni admin). Los hotfixes entran por `hotfix/*` a `main` con backport
  obligatorio (R-GE-5/6); la configuración de apps por entorno en Dockploy queda
  documentada en el guion operativo `docs/dockploy-setup.md` (Etapa D3).
- **Mitigación / pendiente**: si en el futuro se desea un pipeline de despliegue
  propio, R-CD-6/11 exigen pasar por `environment` con revisores; el gate de
  producción hoy es la promoción + verificaciones (R-CD-10/11).

---

## ADR-13: Entornos Dockploy — un proyecto, dos entornos, desarrollo local

- **Fecha**: 2026-10-01
- **Estado**: Aceptado
- **Contexto**: el plan inicial del modelo CD preveía **tres aplicaciones
  Dockploy** (desarrollo/stage/producción), replicando la pila 3 veces en el VPS:
  coste fijo innecesario y ajeno al patrón habitual (el entorno de desarrollo no
  se despliega). La revisión de fuentes de CD (continuousdelivery.com →
  deployment pipeline y "deploy the same way to every environment";
  Atlassian → trunk-based/GitFlow; análisis de coste staging vs entornos
  efímeros) concluye que para 1–2 devs lo óptimo es **desarrollo local + un
  checkpoint de staging + producción**, con infraestructura compartida y
  aislamiento lógico ("nunca un recurso físico por entorno").
- **Decisión**: un **solo proyecto Dockploy `core-engine` con dos environments** —
  `staging` (rama `staging`) y `produccion` (rama `main`) — usando el
  multi-tenancy nativo de Dockploy (variables y red por environment). El
  **desarrollo corre local** (`docker compose -f docker-compose.dev.yml up` + `npm run demo`) y `develop`
  queda como rama de integración **sin despliegue**. Infraestructura compartida:
  un Postgres con una base por entorno y un RabbitMQ con un vhost por entorno;
  monitoreo solo en producción (+ opcional staging).
- **Consecuencias**: `R-CD-9` enmendada (proyecto + environments, no app por
  rama); `R-GE-1` aclarada (develop = integración/local); guion operativo
  `docs/dockploy-setup.md` reescrito al modelo de 2 environments; la puerta de
  seguridad (CI 4/4 → staging smoke → promoción a producción) no cambia.

---

## ADR-14: Historiales de entorno divergentes — no realinear; promoción por delta (R-GE-7)

- **Fecha**: 2026-10-01
- **Estado**: Aceptado
- **Contexto**: `develop`, `staging` y `main` tienen el **mismo contenido pero
  linajes distintos**: el contenido del modelo CD (D1/D4 y el modelo A) viajó al
  pipeline por caminos separados (merges desde `develop` + recreaciones de
  promoción), de modo que GitHub responde `This branch can't be rebased` en cada
  promoción entre ramas de entorno y el paso de recreación del delta fue
  necesario en todos los saltos.
- **Decisión**: **no** se realinean los historiales (desproteger `staging`/`main`
  + force-push) porque violaría `R-GP-3`/`R-GE-1` y reescribir el historial de
  una rama de entorno no aporta contenido. En su lugar, cada promoción aplica el
  paso de **R-GE-7**: rama nueva sobre la rama destino + cherry-pick del delta +
  verificación de **árbol idéntico** (`git diff --quiet <origen>` = vacío) antes
  de reabrir el PR. Una realineación futura solo se hace por enmienda explícita.
- **Consecuencias**: cada promoción cuesta ~2–3 min extra (recreación +
  verificación de árbol); el contenido entre ramas de entorno es idéntico en
  cada salto (probado con `git diff --quiet`); el procedimiento está normado en
  **R-GE-7** y queda cubierto por los checks de CI en cada PR.

---

## ADR-15: Compose base + observabilidad opcional; valores dev/prod separados

- **Fecha**: 2026-10-02
- **Estado**: Aceptado (ampliado por ADR-22)
- **Contexto**: Dockploy ejecuta `docker compose -f ./docker-compose.yml` con el
  `.env` del entorno. El compose único incluía la observabilidad (Prometheus,
  Grafana, exporters) y `grafana` exigía `GRAFANA_ADMIN_PASSWORD` con `:?`, así
  que un despliegue de la **aplicación** fallaba por credenciales de dashboards
  que no aplican a producción. Además mezclaba valores de desarrollo
  (`.env.example` con `core_engine_dev`, `guest:guest`, secretos dev) con
  producción, y los defaults del compose (`core-engine`) no coincidían con
  `.env.example` (`core_engine`).
- **Decisión**:
  1. **`docker-compose.yml` = base** (app + Postgres + RabbitMQ): solo exige
     `POSTGRES_PASSWORD` (+ variables de runtime del `.env`).
  2. **`docker-compose.observability.yml` = overlay opcional** (Prometheus,
     Grafana, exporters): se activa con `-f docker-compose.observability.yml` y
     solo entonces exige `GRAFANA_ADMIN_PASSWORD` y `RABBITMQ_PASSWORD`.
  3. **Plantillas de valores separadas**: `.env.example` (desarrollo) y
     `.env.production.example` (producción, con `<CAMBIAR>` y secretos únicos).
  4. Alinear los defaults del compose con `.env.example` (`core_engine`).
- **Consecuencias**: Dockploy despliega la app sin requerir credenciales de
  dashboards; dev y producción tienen plantillas y secretos separados; la
  observabilidad es explícita. **Deuda anotada**: la credencial de RabbitMQ sigue
  fija en `infra/rabbitmq/rabbitmq.conf` (no es paramétrica por entorno); moverla
  a variable de entorno es trabajo pendiente.

---

## ADR-16: Producción publica solo el gateway; puertos de datos/broker interna

- **Fecha**: 2026-10-05
- **Estado**: Aceptado (revisado por ADR-22)
- **Contexto**: el compose base publicaba al host `5432` (Postgres), `5672` y
  `15672` (RabbitMQ) además del `gateway` (8080). Esos puertos no los necesita
  ningún componente de producción (los servicios los alcanzan por la red interna
  `core-engine`), y exponerlos al host amplía innecesariamente la superficie de
  ataque.
- **Decisión**: el `docker-compose.yml` base (válido para stage/producción)
  **solo publica `gateway:8080`**. Los puertos de datos/broker para desarrollo
  (`5432`, `5672`, `15672`) se mueven a un overlay **`docker-compose.dev.yml`**
  que no se usa en producción. La capa de observabilidad sigue en
  `docker-compose.observability.yml` (opcional; añade `grafana:3000`).
- **Consecuencias**: producción/staging exponen un único puerto; en local se
  abre la BD/broker con `docker compose -f docker-compose.yml -f docker-compose.dev.yml up`.
  Ningún microservicio de dominio es accesible desde fuera de la red interna.

---

## ADR-17: `commissions-service` y `market-intelligence-service` fuera de producción (temporal)

- **Fecha**: 2026-10-05
- **Estado**: Aceptado (temporal) — mecanismo de overlay sustituido por ADR-22
- **Contexto**: por ahora no se quieren desplegar `commissions-service` ni
  `market-intelligence-service` en producción; el `docker-compose.yml` base los
  levantaba junto al resto.
- **Decisión**: **sacar sus definiciones del compose base** (`docker-compose.yml`)
  y llevarlas a un overlay **`docker-compose.extra.yml`** que no se usa en
  producción; se activan con `docker compose -f docker-compose.yml -f
  docker-compose.extra.yml up`. Al **no estar definidos** en el compose base, el
  `up -d --remove-orphans` de producción **elimina** sus contenedores si venían
  de un despliegue anterior (con `profiles` no se eliminaban: seguían definidos,
  solo deshabilitados, y `--remove-orphans` no los toca).
- **Consecuencias**: producción levanta gateway + 7 microservicios (identity,
  catalog, stores, orders, logistics, finance, field) + Postgres + RabbitMQ. Las
  rutas del gateway hacia commissions/intelligence responderán **502** mientras
  estén bajados (no se tocan las políticas de ruta; se registra como punto a
  revisar si se alarga). Reversible: volver a mover las definiciones al compose
  base.

---

## ADR-18: Sesiones revocables — logout por `jti` (R-GW-4)

- **Fecha**: 2026-10-06
- **Estado**: Aceptado
- **Contexto**: la autenticación era **JWT stateless** sin persistencia: no
  había logout ni forma de revocar el refresh token, aunque **R-GW-4** exige
  ("los refresh tokens pueden revocarse cerrando sesión"). El access token
  caducaba a los 900 s, pero un refresh filtrado era válido 7 días sin remedio, y
  cambiar la contraseña no invalidaba nada.
- **Decisión**: persistir cada sesión emitida en **`identity.sesiones`**,
  identificada por el **`jti`** (uuid) incluido en el refresh token:
  - `login`/`registro` crean la sesión; `refresh` solo renueva si la sesión sigue
    activa (no revocada y no expirada).
  - `POST /api/v1/auth/logout` (autenticado, cualquier rol) revoca la sesión del
    `refresh_token` presentado o, si se omite, **todas** las del usuario.
  - `cambiar-contrasena` revoca **todas** las sesiones (obliga a re-autenticar).
  - Soft delete por `revocada_en` (R-DB-5); DDL canónico en
    `infra/db/init/08_auth_sesiones.sql` (R-DB-6).
- **Consecuencias**: el logout sí invalida el refresh (el access sigue válido
  hasta su TTL de 900 s — lo que se revoca es el refresh, como pide R-GW-4). El
  enrutado sigue **stateless** en el gateway (no consulta la BD) y no añade
  latencia. No hay **rotación** de refresh en esta entrega (misma `jti` al
  renovar); la rotación y el cierre selectivo de sesiones desde un listado quedan
  como deuda si se piden. Sobre una BD ya inicializada el DDL debe aplicarse una
  vez a mano (los scripts de `init/` solo corren en un volumen vacío); es
  idempotente.

---

## ADR-19: Dependencias de desarrollo — poda en la imagen y riesgo aceptado del tooling de test

- **Fecha**: 2026-10-14
- **Estado**: Aceptado
- **Contexto**: la puerta de seguridad de CI (`npm audit --audit-level=high`,
  R-QA-6) reportó **1 critical** (Handlebars, vía `ts-jest`) y **20 moderate**,
  todas en la cadena de **test** (`jest`/`ts-jest`/`babel`/`js-yaml`/`argparse`/
  `sprintf-js`). El critical se cerró fijando `handlebars@^4.7.10` por `overrides`
  (bump no rompiente en 4.x). Las 20 moderate **no tienen fix sin un cambio mayor**
  (el `fixAvailable` que propone npm apunta incluso a *bajadas* de `jest`), y
  `npm audit fix` **no reduce** el número pero reescribe miles de líneas del lock
  sin beneficio. Además, el `Dockerfile` copiaba **todo** `node_modules` al runtime,
  incluidas esas devDependencies.
- **Decisión**:
  1. **Podar el runtime**: las imágenes de servicio ejecutan `npm prune --omit=dev`
     tras compilar, de modo que el tooling de test **no viaja** en la imagen
     publicada (R-CD-7/R-CD-22). Aplica al `Dockerfile` raíz, a la plantilla y a
     los `packages/<servicio>/Dockerfile`.
  2. **No forzar** un major de `jest`/`babel` solo por moderates **dev-only**; el
     gate R-QA-6 se mantiene en `--audit-level=high` (bloquea `high`/`critical`).
  3. Registrar el riesgo residual como **aceptado** y acotado a dependencias que
     **no se despliegan**.
- **Consecuencias**: el `node_modules` del runtime baja de ~190 MB a ~85 MB, se
  elimina superficie de ataque en la imagen (no hay `jest`/`babel` en producción) y
  el gate sigue bloqueando lo que importa. Los 20 moderate quedan como deuda
  **visible** ligada al runner de tests; si se adopta `Vitest` (como en el
  microservicio de almacenamiento) o un major de `jest` que los resuelva, se
  revisará. Verificado con `docker build` real de `orders-service` y
  `finance-service`.

---

## ADR-20: Migración del runner de pruebas de Jest a Vitest

- **Fecha**: 2026-10-14
- **Estado**: Aceptado
- **Contexto**: las suites corrían con **Jest 30 + ts-jest** en los 10 workspaces.
  Esa cadena arrastraba deuda de seguridad **dev-only** (20 moderate, ADR-19) y un
  critical en `handlebars`; `npm audit fix` no los resolvía sin cambios mayores
  (bajadas de `jest`), y el árbol de dependencias era enorme (~1200 paquetes).
- **Decisión**: migrar a **Vitest 4** (compatible con Node 20 del CI):
  - Config **por workspace** en `packages/*/vitest.config.ts` (antes `jest` en
    `package.json`): specs `*.spec.ts`, entorno node, alias `@core/shared`,
    cobertura **v8** con `thresholds` por servicio.
  - API de test `jest.*` → `vi.*`; `jest.Mock` → `Mock` (`vitest`).
  - **SWC** (`unplugin-swc`) para emitir metadatos de decoradores: esbuild (por
    defecto en Vite/Vitest) no los emite y la **DI de NestJS** (`Test.createTestingModule`)
    rompía. Afectaba a 2 specs de `market-intelligence-service`.
  - `require()` dinámico → `import` estático (Vitest no resuelve `require` en ESM).
  - Excluir `src/**/*.spec.ts` del `tsconfig` de build en los workspaces que no lo
    hacían (evita que los specs viajen en `dist/`).
  - El job `test` de CI pasa a `npm test` (el script raíz ya corre cada workspace
    con `--coverage`; el `-- --coverage` extra duplicaba el flag y Vitest lo rechaza).
- **Consecuencias**: `npm audit` baja a **0 vulnerabilidades** (desaparecen las 20
  moderate y el critical de `handlebars`); el árbol de deps se reduce a ~377
  paquetes; **385 tests** en verde. Los **umbrales de cobertura se recalibraron**
  porque el proveedor y el alcance cambian (ahora instrumenta todos los ficheros
  del alcance, como hacía `collectCoverageFrom`), según R-COV-4 y la tabla de
  `test/06-estandares-cobertura.md`. Esta decisión **supersede** la parte de
  devDependencies de ADR-19 (el riesgo aceptado de jest/babel deja de existir).

---

## ADR-21: Credencial de RabbitMQ por entorno (sin secreto en el repo)

- **Fecha**: 2026-10-14
- **Estado**: Aceptado
- **Contexto**: `infra/rabbitmq/rabbitmq.conf` (versionado) fijaba
  `default_user`/`default_pass` con una credencial de **desarrollo**, y los tres
  composes (dev, stage y prod) montaban ese mismo fichero → la credencial de
  RabbitMQ era **idéntica en todos los entornos y estaba commiteada**. La deuda
  estaba anotada en ADR-15 y en `dockploy-setup.md` §5.
- **Decisión**: retirar `default_user`/`default_pass` del conf (que conserva solo
  el hardening: `loopback_users.guest=false`, `default_vhost`) e inyectar la
  credencial por entorno desde `RABBITMQ_USER`/`RABBITMQ_PASSWORD` vía
  `RABBITMQ_DEFAULT_USER`/`RABBITMQ_DEFAULT_PASS` en el servicio `rabbitmq` de
  `docker-compose.yml`, `docker-compose.dev.yml` y `docker-compose.stage.yml`
  (ADR-22).
- **Consecuencias**: la credencial es **única por entorno** y ya **no hay un
  secreto commiteado** (`R-CD-7`); dev no cambia (toma el valor de su `.env`).
  En un volumen ya inicializado, `RABBITMQ_DEFAULT_*` **no** re-crea el usuario
  (RabbitMQ solo crea el usuario por defecto en un volumen vacío): para aplicar
  una credencial nueva en un entorno existente hay que rotarla por la gestión de
  RabbitMQ o recrear el volumen. Cierra la deuda marcada en ADR-15.

---

## ADR-22: Un solo compose por entorno (dev/stage/prod); producción = `docker-compose.yml`

- **Fecha**: 2026-10-14
- **Estado**: Aceptado
- **Contexto**: existían **cuatro** archivos de "entorno" y dos overlays. El
  `docker-compose.yml` era una **base** que construía las imágenes en local (y que
  dev usaba vía overlay de puertos), mientras que `docker-compose.prod.yml` y
  `docker-compose.stage.yml` eran **idénticos entre sí** salvo el tag `:prod`/`:stage`
  y el `env_file`, y a su vez **duplicaban la estructura completa** de la base
  (mismo conjunto de servicios; sólo cambiaba `build:` por `image:`). Cualquier
  cambio en un servicio había que replicarlo en 2–3 archivos, y `commissions-service`
  / `market-intelligence-service` vivían aparte en `docker-compose.extra.yml`.
- **Decisión**:
  1. **`docker-compose.yml` = producción**: asume el contenido del antiguo
     `docker-compose.prod.yml` (imágenes GHCR `:prod`, `env_file: .env.production`,
     sin `build:`).
  2. **`docker-compose.stage.yml` = pre-producción** (GHCR `:stage`, `.env.stage`),
     sin cambios.
  3. **`docker-compose.dev.yml` = desarrollo autocontenido**: construye local
     (`build:`, `env_file: .env`), publica `8080` + `5432`/`5672`/`15672` e incluye
     los **10 servicios** (absorbe el antiguo `docker-compose.extra.yml`).
  4. Se **eliminan** `docker-compose.prod.yml` y `docker-compose.extra.yml`.
  5. `docker-compose.observability.yml` sigue siendo una **capa opcional** que se
     añade con `-f` a dev o a producción.
  6. `deploy.yml` resuelve: dev → `-f docker-compose.dev.yml` (`--build`), stage →
     `-f docker-compose.stage.yml`, prod → `-f docker-compose.yml` (sin `build`).
     El test de reglas `R-CD-3` verifica que prod/stage no contengan `build:` y sí
     las imágenes GHCR con `pull_policy: always`; dev debe tener `build:` y los 10 servicios.
- **Consecuencias**: quedan **tres entornos = tres archivos** (dev, stage, prod),
  sin la duplicación base/prod. `R-CD-3` se mantiene intacto (prod/stage consumen
  GHCR sin reconstruir). Supersede parcialmente **ADR-15/16/17**: el "compose base"
  pasa a ser el de producción, los puertos de datos/broker viven en el entorno dev
  (ya no como overlay) y el overlay `extra` desaparece. **Trade-off asumido**: prod
  y stage siguen compartiendo la lista de servicios entre sí; se aceptan como
  entornos separados con imágenes y variables distintas.

---

## Resumen de cumplimiento post-corrección

| Ítem del documento          | Estado  | Nota |
|----------------------------|---------|------|
| RN-01 (margen 0-90)        | ✅      | helper y servicios alineados |
| RN-02 (oferta agotada)     | ✅      | consumidor `stock.updated` vivo |
| RN-03 (descuento atómico)  | ✅      | transacción catalog + evento |
| RN-04 (comisión 12 %)      | ✅      | `Money.comision(0.12)` |
| RN-05 (carrito 30 min)     | ✅      | tabla + endpoints + expiración |
| RN-06 (devolución stock)   | ✅      | `stock.reintegrado` + `stock.updated` |
| RN-07 (liquidación 1/15)   | ✅      | `LIQUIDACION_DIAS = [1,15]` |
| RN-08 (histórico precios)  | ✅      | `historico_precios` en catalog y stores |
| Tabla 13 (ciclo orden)     | ✅      | `puedeTransicionar` + handler |
| Tabla 21 (10 endpoints)    | ✅      | + carrito endpoints |
| Tabla 22 (eventos)         | ✅      | nombres exactos; `stock.updated` extendido |
| AD-02 (RabbitMQ + DLQ)     | ✅      | exchanges + colas + DLQ |
| AD-03 (Outbox)             | ✅      | tabla por servicio + poller |
| AD-04 (backoff DLQ)        | ✅      | poller 10s + reinyección |
| 5.3 (métricas + request_id)| ✅      | todos los servicios |
| 5.4 (gateway rate limit)   | ✅      | 429 real + cabeceras |
| 5.7 (errores estructurados)| ✅      | normalizador universal |
| 4.3/RLS (seguridad)        | ✅      | esquemas + políticas nuevas |

---

*Generado tras auditoría 2026-08-09. Todas las correcciones verificadas con `npm run build`, `npm run test`, `npm run lint`.*