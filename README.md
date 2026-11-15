# Core Engine

**Plataforma multi-vertical por capacidades** (microservicios en Rust y
PostgreSQL/Supabase). Un mismo backend opera negocios de rubros distintos: una
empresa nueva elige una **vertical**, adopta un **pack** versionado y activa sus
**capacidades**; no hay microservicios por rubro ni cambios de código para
arrancar un negocio nuevo.

La decisión que rige la arquitectura es
[**ADR-0001 · Plataforma multi-vertical por capacidades**](docs/adr/0001-plataforma-multi-vertical.md)
(ADR-44 en el histórico de `docs/decisiones.md`). El documento
**Core Engine_Informe.docx v1.6** acota el dominio comercial original; el modelo
por capacidades lo generaliza a retail, farmacia, belleza, dental, flota,
talleres y verticales que se definan por configuración.

Límites deliberados (ADR-0001): **no** es un ERP universal ni un motor de
workflow genérico, y `historia_clinica` / `recetas` quedan fuera de la primera
fase.

Requiere **Node >= 20 (uso v24)**, **Rust estable** y **Docker con Compose v2**.
Todas las rutas públicas van por el gateway (`http://localhost:8080/api/v2/...`).
Los montos se manejan como **enteros en centavos** (control OWASP A02): el JSON
entrega strings con dos decimales (`"1150.00"`).

## Modelo multi-vertical (ADR-0001)

```text
vertical ─────────▶ pack (versionado) ─────────▶ capacidades ─────▶ empresa (tenant)
retail              pack_retail v1                catalogo_items      TechFix
farmacia            pack_farmacia v1              inventario_lotes    ...
belleza             pack_belleza v1               agenda_recursos
dental              pack_dental v1                logistica_rutas
flota               pack_flota v1                 cotizaciones
tecnologia_         pack_tecnologia_              facturacion
reparacion *        reparacion v1                 liquidaciones
```

\* La vertical de tecnología y reparación es un **blueprint de configuración**,
no código nuevo: [`docs/blueprints/tecnologia-reparacion.md`](docs/blueprints/tecnologia-reparacion.md)
compone las piezas existentes (ítems, inventario, agenda, cotizaciones,
logística, crédito y registros).

### Verticales sembradas

| Vertical | Modelo de operación |
|---|---|
| `retail` | Venta de productos con inventario y canales de entrega |
| `farmacia` | Venta de productos con lotes, vencimiento y trazabilidad |
| `belleza` | Venta de servicios con agenda y recursos reservables |
| `dental` | Servicios agendados; capacidades clínicas en fase posterior |
| `flota` | Vehículos, rutas y servicios cotizables |

### Capacidades

| Capacidad | Módulo | Habilita |
|---|---|---|
| `catalogo_items` | catalog | Productos, servicios y alquileres |
| `inventario_stock` | catalog | Stock, mínimos/máximos y movimientos |
| `inventario_lotes` | catalog | Lotes, vencimiento y consumo FEFO |
| `codigo_barras` | catalog | Etiquetas y escaneo en mostrador |
| `agenda_recursos` | scheduling | Recursos, disponibilidad, citas y bloqueos |
| `fulfillment_delivery` | logistics | Entrega con flota de plataforma |
| `fulfillment_pickup` | logistics | Retiro en tienda |
| `fulfillment_onsite` | logistics | Servicio a domicilio |
| `logistica_rutas` | logistics | Flota propia: vehículos, rutas y paradas |
| `cotizaciones` | orders | Presupuesto → aceptación → orden |
| `facturacion` | finance | Facturas, comprobantes y régimen fiscal |
| `liquidaciones` | commissions | Comisiones y liquidaciones |
| `operacion_campo` | field | Personal, visitas, tracking e incidencias |
| `inteligencia_mercado` | intelligence | Demanda, tendencias, forecast y score |
| `historia_clinica` / `recetas` | clinical | Fuera de la primera fase |

Los servicios **siempre activos** (`identity-service`, `api-gateway`,
`storage-service`, `notifications-service`) no se gatean. Los servicios de
feature validan `CapabilityGuard` antes de operar: una capacidad inactiva
devuelve el error estructurado `CAPACIDAD_NO_ACTIVA` y sus rutas no se anuncian
en el pack del tenant. El aislamiento de datos entre tenants es por **RLS**
(esquema por dominio + `empresa_id`).

### Alta de un tenant nuevo

```bash
# 1) (Opcional) registrar una vertical y su pack versionado
curl -X POST http://localhost:8080/api/v2/packs/verticales \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"codigo":"tecnologia_reparacion","nombre":"Tecnología y reparación","modelo_operacion":"mixto"}'

# 2) consultar verticales, packs y capacidades disponibles
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v2/packs/verticales
curl -H "Authorization: Bearer $TOKEN" "http://localhost:8080/api/v2/packs?vertical_id=tecnologia_reparacion"

# 3) crear la empresa eligiendo vertical + pack
curl -X POST http://localhost:8080/api/v2/empresas \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"nombre":"TechFix","slug":"techfix","vertical_id":"tecnologia_reparacion",
       "pack_codigo":"pack_tecnologia_reparacion","pack_version":1}'

# 4) verificar capacidades efectivas del tenant
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v2/empresas/<empresaId>/capacidades
```

Migración de tenants existentes, pilotos y rollback:
[`docs/runbook-multi-vertical.md`](docs/runbook-multi-vertical.md).

## Arquitectura

```text
                    ┌─────────────┐   TLS/CORS/Throttling en el borde
  Cliente ─────────▶│ API Gateway │  JWT HS256 (access 15 min + refresh 7 d)
                    │   :8080     │  Politicas por ruta/rol (Tabla 21) · UNICO puerto al host
                    └──────┬──────┘
                           │  red interna Docker `core-engine` (microservicios sin puerto al host)
  identity  catalog  inventory  stores  orders  logistics  scheduling  commissions  finance  field  intelligence  storage  notifications
    :3001    :3002    :3013     :3003   :3004     :3005      :3011       :3006      :3007   :3008     :3009      :3010      :3012
  (commissions solo stage · market-intelligence solo dev)
       RabbitMQ · topic core-engine.events (+ DLQ, AD-04) + Postgres 16 + MinIO (un bucket por tenant, ADR-24)
```

Los bounded contexts se organizan **por capacidad de negocio**, no por rubro:
`identity` es la autoridad de tenants, roles, verticales, capacidades y packs;
`catalog`, `stores`, `orders`, `scheduling`, `logistics`, `finance`,
`commissions` y `field` habilitan las capacidades del pack; `storage`,
`notifications` y `intelligence` son transversales. No existen
`dentista-service`, `farmacia-service` ni `pulperia-service` (ADR-0001).

**Punto de entrada unico:** solo el servicio `gateway` publica un puerto de aplicación al host
(`8080:8080`); los **trece microservicios** escuchan en `3001`–`3013` **solo dentro de la red interna
Docker `core-engine`** y no tienen mapeo de puertos al host. La excepción es **MinIO** (`9000`), que se
publica en **loopback** porque es el host local que Traefik firma como dominio HTTPS en las URLs
prefirmadas (ADR-24). Toda llamada externa pasa por `http://localhost:8080/api/v2/...` (o el dominio
del gateway en stage/produccion).

- **CQRS + Event Sourcing** solo en `orders` (alta tasa de escritura, cap. 3.2); el resto usa
  CRUD convencional sobre su propio esquema (baja tasa de escritura).
- **Outbox pattern** (ADR-03): cada servicio publica sus eventos en `x.outbox` en la misma
  transaccion; un reenviador los mueve al broker. RabbitMQ confirma publicacion (confirm channel).
- **Saga** (ADR-02): `order.created` → catalog descuenta stock (atomico) → `stock.reservado` →
  orders marca `pagada` y emite `payment.procesado` → logistics crea el envio → ...
  `stock.fallido` revierte la orden a `cancelada`.
- **DLQ con backoff exponencial** (AD-04): reintentos hasta `REINTENTOS_MAXIMOS`; luego el
  mensaje se declara muerto. `POST /api/v2/admin/...dlq...` (endpoint interno) permite reinyectar.
- **Ciclo de vida de la orden** (Tabla 13): `creada → pagada → en_preparacion → enviada →
  entregada` (y `cancelada` / `devuelta`); las transiciones se validan con `validarTransicion`.
- **Capacidades por tenant** (ADR-0001): vertical + pack versionado + `empresa_capacidades`;
  `CapabilityGuard` aplica el gating en cada servicio y el gateway solo autentica y enruta.

## Reglas de negocio del dominio comercial

Las reglas RN del documento original siguen vigentes en los packs que activan las
capacidades comerciales (`catalogo_items`, `inventario_stock`, `orders`,
`facturacion`, `liquidaciones`). Una vertical que no venda productos puede
desactivarlas por capacidad sin tocar código.

| Regla | Descripcion |
|---|---|
| RN-01 | Precio final = precio base × (1 + margen), margen entero 0–90 % |
| RN-02 | No se publica oferta con stock < 1 |
| RN-03 | Descuento atomico de stock; rechazo total si alguna linea no alcanza (409 rápido) |
| RN-04 | Comision 12 % (configurable); el vendedor recibe venta − comision |
| RN-05 | Carrito de 30 min en cliente; no reserva stock; cantidades repetidas se agrupan |
| RN-06 | Devolucion revierte stock y compensa la comision en la siguiente liquidacion |
| RN-07 | Liquidaciones quincenales (dias 1 y 15, cron America/Managua) |
| RN-08 | Cambios de precio base van al historico; aplican solo a ofertas nuevas |

Eventos del bus (topic `core-engine.events`): `order.created`, `stock.reservado`,
`stock.fallido`, `stock.reintegrado`, `payment.procesado`, `order.completado`,
`comision.acreditada`, `devolucion.solicitada`, `shipment.started`, `stock.updated`,
`order.status.updated`, `declaracion.generada`.

## Puesta en marcha

> **Enlaces de paquete:** en discos **exFAT/FAT32** (sin soporte de enlaces)
> `npm install` falla con `EISDIR`; mueva el repositorio a un sistema de archivos
> con symlinks (ext4/APFS/NTFS). La instalación es siempre con npm (R-EN-1).

```bash
npm install                       # tooling Node de los scripts de gobernanza (ESLint, pg)
npm run lint                      # lint de los scripts de gobernanza
npm run test:engine               # unitarias del motor de reglas
npm run test:smoke                # unitarias de los scripts de smoke
npm run rules:check               # verificación de reglas del repositorio
cp .env.example .env              # variables de DESARROLLO (producción: .env.production.example)
docker compose -f docker-compose.dev.yml up -d --build   # app: Postgres + RabbitMQ + MinIO + Mailpit + gateway y 13 servicios (8080 host + datos/broker 5432/5672/15672 + minio 9000/9001 + mailpit 1025/8025)
npm run demo                      # ejercicio end-to-end (TC-01..TC-08, RN-01..RN-08)
npm run demo:verticals            # E2E de los pilotos belleza y flota contra el gateway real

# Dashboards (opcional): añade Prometheus + Grafana + exporters al stack de desarrollo
docker compose -f docker-compose.dev.yml -f docker-compose.observability.yml up -d --build
```

Cada paquete Rust se verifica con su propio crate:

```bash
cd packages/orders-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

La suite API+BD (`npm run test:integration`) usa PostgreSQL efímero real y no
dobles de prueba; requiere `TEST_DATABASE_URL`.

Postgres siembra automáticamente `infra/db/init/01_esquemas.sql` (esquemas, tablas, checks,
seeds del plan contable NIC e indices). En **Supabase** (staging/prod) aplique ademas
`infra/db/init/99_rls.sql`, que activa el aislamiento por rol de Tabla 15 (funciones
`auth.rol()`/`auth.uid()`).

### Servicios

| Servicio | Puerto interno | Expuesto al host | Esquema | Fuente |
|---|---|---|---|---|
| api-gateway | 8080 | **Sí** (dev `8080:8080`; stage/prod `127.0.0.1:8080:8080` tras Traefik) | — | `packages/api-gateway` |
| identity-service | 3001 | No | `identity` | `packages/identity-service` (Rust) |
| catalog-service | 3002 | No | `catalog` | `packages/catalog-service` (Rust) |
| inventory-service | 3013 | No | `inventory` | `packages/inventory-service` (Rust, ADR-0018) |
| stores-service | 3003 | No | `stores` | `packages/stores-service` (Rust) |
| orders-service | 3004 | No | `orders` | `packages/orders-service` (Rust) |
| logistics-service | 3005 | No | `logistics` | `packages/logistics-service` |
| scheduling-service | 3011 | No | `scheduling` | `packages/scheduling-service` |
| commissions-service | 3006 | No *(solo stage; fuera de prod)* | `commissions` | `packages/commissions-service` (Rust) |
| finance-service | 3007 | No | `finance` | `packages/finance-service` |
| field-service | 3008 | No | `field` | `packages/field-service` |
| market-intelligence-service | 3009 | No *(fuera de stage/prod)* | `intelligence` | `packages/market-intelligence-service` |
| storage-service | 3010 | No | `storage` | `packages/storage-service` |
| notifications-service | 3012 | No | `notifications` | `packages/notifications-service` |
| postgres | 5432 | No *(dev: `docker-compose.dev.yml`)* | todos | `infra/db/init/` |
| rabbitmq (+ management) | 5672 / 15672 | No *(dev: `docker-compose.dev.yml`)* | — | — |
| minio (+ consola) | 9000 / 9001 | Solo loopback *(el público es su dominio HTTPS, ADR-24)* | — | `docker-compose*.yml` |
| mailpit *(capturador SMTP, solo dev)* | 1025 / 8025 | Solo loopback | — | `docker-compose.dev.yml` |
| prometheus *(capa opcional)* | 9090 | No (solo red interna) | — | `infra/prometheus/` |
| grafana *(capa opcional)* | 3000 | Sí (`3000:3000`) | — | `infra/grafana/` |

> **Aislamiento de red:** en producción/staging **solo `gateway` (8080) y `minio` (9000, API)** publican
> puerto al host (`grafana` 3000 solo si se activa la capa de observabilidad). El 9000 de MinIO es el
> host **firmado** en las URLs prefirmadas (ADR-24); su consola (9001) nunca se publica. Postgres y
> RabbitMQ son **internos**;
> sus puertos al host se abren únicamente en desarrollo con `docker-compose.dev.yml`. **Ningún
> microservicio de dominio es accesible desde fuera**: se alcanzan por nombre de servicio
> (`http://<servicio>:<puerto>`) dentro de la red `core-engine`, detrás del gateway.
>
> **Alcance por entorno:** `commissions-service` se despliega en **stage** para validar el
> portal de empresa (`docker-compose.stage.yml`, ADR-37), pero **sigue fuera de producción**
> (`docker-compose.yml`). `market-intelligence-service` solo se levanta en desarrollo. En
> producción, sus rutas del gateway responderán 502 mientras estén bajados.

## Entornos y entrega (desarrollo → stage → producción)

Tres entornos por rama protegida, desplegados por **Dockploy** en el VPS:

| Etapa | Rama | Dónde corre |
|---|---|---|
| Desarrollo | `develop` | **local** (`docker compose -f docker-compose.dev.yml up`) — nunca se despliega |
| Stage | `staging` | Dockploy (environment `staging`) |
| Producción | `main` | Dockploy (environment `produccion`) |

Flujo: `feature/xxx` → PR → `develop`; luego PR `develop → staging`; y solo
cuando stage está verificado (CI verde + smoke) PR `staging → main`. **A `main`
solo se llega por promoción**: es la frontera de producción y Dockploy redepliega
sus cambios automáticamente — el modelo de ramas es la garantía de que no se
rompe producción. Dockploy aloja **un solo proyecto con dos environments**
(`staging`/`produccion`) e infraestructura compartida (una BD y un vhost por
entorno). Modelo completo en `docs/rules/git/00-entornos.md` (R-GE) y despliegue
en `docs/rules/cd/` (R-CD).

## API (superficie pública `/api/v2`, por el gateway)

La **fuente de verdad** es el contrato OpenAPI versionado, no esta tabla:
`docs/api/core-engine.openapi.json` (público) y
`docs/api/core-engine.internal.openapi.json` (interno, `x-internal-key`).
Toda ruta pública empieza por `/api/v2/` y toda ruta interna por `/internal/`
(R-DO-8).

| Área | Rutas principales |
|---|---|
| auth | `/auth/{registro,login,refresh,logout,me,token}` |
| tenancy | `/empresas/**` (alta con `vertical_id` + `pack_codigo`, perfil, slug, roles, puestos, personal, partes, registros, extensiones y capacidades) |
| packs | `/packs`, `/packs/verticales`, `/packs/modelos-operacion`, `/packs/{codigo}/{version}` |
| catálogo | `/catalog/items/**` (productos, servicios, alquileres, variantes, imágenes, tarifas, escalas, componentes, series), `/catalog/listas-precio/**`, `/catalog/combos`, `/catalog/promociones`, `/catalog/inventario/movimientos` |
| tiendas | `/tiendas/{id}` + logo, `tema.css`, background, contenido, personalización y slug |
| órdenes | `/orders/**`, `/carrito/**`, `/pedidos/{id}/entregas-digitales`, `/entregas-digitales/{token}`, `/contratos/**`, `/suscripciones/**`, `/mesas`, `/comandas` |
| agenda | `/scheduling/{recursos,disponibilidad,bloqueos,citas,recordatorios}` |
| logística | `/envios/{id}/estado`, `/empresarios/me/envios`, `/field/**` (personal, vehículos, rutas, visitas, tracking, incidencias, cumplimiento y sync) |
| finanzas | `/finanzas/**` (cuentas, asientos, libros, comprobantes, regímenes, declaraciones, proyecciones, KPIs y tablero) |
| comercio | `/comercio/**` (bancos, créditos, gastos, cuentas por pagar, proveedores, compras, órdenes de compra, caja, clientes y códigos de barras) |
| comisiones | `/vendedores/me/{ventas,liquidaciones,fiscal,tienda,ofertas}`, `/empresarios/me/{ventas,liquidaciones,tiendas}`, `/admin/liquidaciones/**` |
| administración | `/admin/**` (inventario, lotes, ubicaciones, reportes, envíos y storage por tenant) |
| inteligencia | `/inteligencia/**` (mapa de calor, rendimiento, cobertura, demanda, tendencias, forecast, anomalías y score) |
| storage | `/storage/**` (buckets, subidas, objetos y media pública) |
| operación | `GET /health` (liveness), `GET /ready` (readiness agregada: 10 servicios en producción, 11 en stage), `GET /metrics`, Swagger en `/docs` y `/docs-json` |

Endpoints internos (servicio→servicio) protegidos con `x-internal-key`:
`internal/items/**`, `internal/inventario/**`, `internal/usuarios/:id`,
`internal/vendedores`, `internal/empresas/**`, `internal/partes/:id`,
`internal/orders/**`, `internal/scheduling/disponibilidad`,
`internal/storage/**`, `internal/ofertas/**`, `internal/tiendas/**` e
`internal/finance/**`.

## Contrato OpenAPI (Swagger)

Cada microservicio monta Swagger UI en **`/docs`** (su `main.ts` usa
`crearDocumentoOpenApi`) y versiona sus dos superficies en
`docs/openapi/<servicio>/public.openapi.json` e `internal.openapi.json`.

El spec se regenera y se verifica en CI con el **gate G-8** (`R-C-7`): el job *Build*
ejecuta `npm run swagger:export` y falla si `docs/openapi`, `docs/api` o el
agregado del gateway cambian sin commitear (`git diff --exit-code`). Al tocar un
DTO hay que anotarlo con `@ApiProperty`.

El **API Gateway** sirve en su `/docs` el **documento agregado de la superficie
pública**: `scripts/tools/swagger-aggregate.mjs` fusiona
`docs/openapi/<servicio>/{public,internal}.openapi.json` y escribe dos artefactos:

- `docs/api/core-engine.openapi.json` — contrato público (solo `/api/v2/**`),
  versionado y verificado por el gate G-8, que el gateway sirve en `/docs-json`;
- `docs/api/core-engine.internal.openapi.json` — contrato interno (solo
  `/internal/**`, con `x-internal-key`), que **nunca** se expone por el gateway.

| Cómo obtenerlo | Dónde |
|---|---|
| Contrato público versionado (recomendado para frontend) | `docs/api/core-engine.openapi.json` — raw en GitHub: `…/raw/<rama>/docs/api/core-engine.openapi.json` |
| Contrato interno versionado (servicio→servicio) | `docs/api/core-engine.internal.openapi.json` |
| Gateway en ejecución (local) | `http://localhost:8080/docs` (Swagger UI) y `/docs-json` (JSON público) |
| Staging/producción | `/docs*` está **privado** (404): descárgalo con `x-internal-key: $INTERNAL_API_KEY` o usa el archivo del repo |

```bash
npm run swagger:export   # regenera docs/openapi/<servicio>/{public,internal} + los dos agregados de docs/api/
```

Cobertura actual: **420/420 operaciones documentadas** en los 13 servicios con
contrato versionado: **381 públicas** (270 rutas) y **36 internas** (todas con
`x-internal-key` y schema de respuesta).

## Variables de entorno (`.env` en dev · `.env.production` en producción)

- **Desarrollo**: copia `.env.example` → `.env`.
- **Producción**: usa `.env.production.example` (secretos reales y únicos por
  entorno; en Dockploy, pestaña **Environment** del proyecto/entorno). `.env`
  está en `.gitignore` y **nunca** se versiona (R-GA-4).

Claves: `DATABASE_URL`/`POSTGRES_*`, `RABBITMQ_URL` (+ `RABBITMQ_USER/PASSWORD`),
`JWT_SECRET` (+ TTLs), `INTERNAL_API_KEY`, `ADMIN_EMAIL/ADMIN_PASSWORD`,
`COMMISSION_RATE`, `OUTBOX_TABLA` (por servicio, ej. `orders.outbox`),
`*_SERVICE_URL` (URLs internas), `CORS_ORIGINS`, `FRONTEND_URL` (enlaces de
invitación), `LOG_LEVEL`, las de almacenamiento (`MINIO_ROOT_USER/PASSWORD`,
`MINIO_ENDPOINT/PORT`, **`MINIO_PUBLIC_ENDPOINT/PORT/USE_SSL`**: host, puerto y
esquema firmados en las URLs prefirmadas; en stage/prod el público es el dominio
HTTPS con `443`/`true`, `STORAGE_*` de cuota/límites, ADR-24), las de borde
(`DIAGNOSTICOS_PUBLICOS`, `READY_PUBLIC`, `TRUST_PROXY`; el control de acceso a
staging vive fuera del gateway, ADR-0005) y, si se activa la observabilidad, `GRAFANA_ADMIN_PASSWORD`.
En stage/producción `RABBITMQ_PASSWORD` es **obligatoria** (el compose la exige
con `${RABBITMQ_PASSWORD:?...}`), no solo `POSTGRES_PASSWORD`.

## Pruebas

`sandbox` de reglas (Tabla 5) y casos de aceptacion cubiertos: TC-01 alta de producto,
TC-02 oferta con margen, TC-03 checkout con descuento de stock, TC-04 stock insuficiente (409),
TC-07 seguridad del gateway (401/403), TC-08 comision 12 % y liquidacion al vendedor,
RN-05/RN-06/RN-07/RN-08, partida doble y regimen fiscal NIC. La matriz
multi-vertical añade `vertical-matrix`, `capability-enforcement`,
`vertical-operations`, `pack-lifecycle`, `vertical-rls`, `capability-routes`,
`vertical-pilots`, `field-rls` y `lot-fefo`.

```bash
npm run test:engine     # unitarias del motor de reglas
npm run test:smoke      # unitarias de los scripts de smoke
npm run test:integration   # API+BD sobre PostgreSQL efímero (TEST_DATABASE_URL)
npm run multi-vertical:check   # tenants sin pack, packs inactivos y migraciones pendientes
cargo test --manifest-path packages/orders-service/Cargo.toml   # unitarias del crate
npm run demo            # flujo real sobre el stack levantado
npm run demo:verticals  # pilotos belleza y flota
```

La cobertura Rust se mide con `cargo llvm-cov` y ratchet por servicio en CI
(`R-COV-1`).

## Observabilidad

La observabilidad es una **capa opcional** (`docker-compose.observability.yml`):
se activa con `docker compose -f docker-compose.dev.yml -f docker-compose.observability.yml up`
y exige `GRAFANA_ADMIN_PASSWORD` en el `.env`. Prometheus scrapea `/metrics` de
los servicios (`infra/prometheus/prometheus.yml`) y Grafana provisiona el
dashboard de Core Engine (latencia p99, peticiones/s, errores 5xx, backlog de
RabbitMQ). Los logs son JSON estructurados con `x-request-id` correlacionado
(AsyncLocalStorage).

## Conmutacion a Supabase (produccion)

1. Cree el proyecto Supabase (Postgres + Auth).
2. Ejecute **todas** las migraciones de `infra/db/init/` en orden (incluidas
   `45_hardening.sql` y `46_commission_payload.sql`) en la consola SQL. El
   manifiesto `scripts/config/db-migrations.json` es la fuente de verdad de los
   hashes (R-DB-6); no edite una migración ya aplicada.
3. Ejecute `99_rls.sql` (activa RLS por rol).
4. Iguale el secreto JWT HS256 y las URL de los servicios al proyecto. La
   diferencia de implementacion queda aislada en `jwt.utils.ts`.

## Contribuir

El proyecto se gobierna por reglas ejecutables en `docs/rules/` y por PR
obligatorio (`R-PR-1..9`, `R-IA-5`): rama semántica desde `develop`, commits
Conventional Commits en español con el ID de regla, y checks verdes en la cabeza
del PR antes de mergear con `--rebase`. Punto de entrada:
[`docs/rules/README.md`](docs/rules/README.md) y
[`docs/rules/_meta/CHECKLIST.md`](docs/rules/_meta/CHECKLIST.md).
