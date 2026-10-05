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
  **desarrollo corre local** (`docker compose up` + `npm run demo`) y `develop`
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
- **Estado**: Aceptado
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
- **Estado**: Aceptado
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
- **Estado**: Aceptado (temporal)
- **Contexto**: por ahora no se quieren desplegar `commissions-service` ni
  `market-intelligence-service` en producción; el `docker-compose.yml` base los
  levantaba junto al resto.
- **Decisión**: marcarlos con `profiles: ["extra"]` en `docker-compose.yml`, de
  modo que **no arrancan** en un `docker compose up` normal (lo que usa
  producción/Dockploy) y se habilitan solo con `docker compose --profile extra up`
  (desarrollo u otros entornos que los necesiten). Se mantienen definidos para
  reactivarlos sin cambios cuando se decida.
- **Consecuencias**: producción levanta gateway + 7 microservicios (identity,
  catalog, stores, orders, logistics, finance, field) + Postgres + RabbitMQ. Las
  rutas del gateway hacia commissions/intelligence responderán **502** mientras
  estén bajados (no se tocan las políticas de ruta; se registra como punto a
  revisar si se alarga). Reversible: quitar el `profiles` y volver a incluirlos.

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