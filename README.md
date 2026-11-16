# Core Engine

**Core Engine es una fábrica de negocios multi-vertical**: una plataforma de
microservicios en **Rust** sobre **PostgreSQL/Supabase** que permite operar
negocios de rubros distintos sobre el mismo backend, sin crear un servicio por
rubro y sin desplegar código para arrancar un negocio nuevo.

La idea central es simple: **el rubro no es código, es configuración**. Una
empresa nueva elige una **vertical** (retail, farmacia, belleza, dental, flota,
taller, restaurante…), adopta un **pack versionado** y activa un conjunto de
**capacidades**. El backend enciende solo las funciones que el pack declara y el
frontend compone la experiencia a partir del **manifiesto de features** de la
empresa.

## La idea en una frase

> Un mismo núcleo de servicios (identidad, catálogo, pedidos, agenda, logística,
> pagos, finanzas, comisiones, analítica) se recombina por configuración para
> operar una tienda, una distribuidora, una farmacia, un salón de belleza, una
> clínica, una flota o un restaurante, sin bifurcar el código.

## El problema que resuelve

Construir software para un rubro suele terminar en uno de dos extremos:

1. **Un producto por rubro**: código duplicado, equipos duplicados y contratos
   que se separan con el tiempo.
2. **Un ERP genérico**: un motor de workflow enorme que obliga a configurar
   todo y no resuelve bien ningún negocio concreto.

Core Engine toma un camino intermedio: **bounded contexts por capacidad de
negocio** y **composición por configuración**. Las reglas transversales
(identidad, permisos, tenancy, eventos, outbox, auditoría, observabilidad) se
escriben una vez; lo que cambia por rubro se expresa como vertical + pack +
capacidades + configuración.

## Cómo lo resuelve

- **Vertical**: el modelo de operación inicial del negocio (retail, farmacia,
  belleza, dental, flota, o una vertical creada por un admin).
- **Pack versionado**: un preset de capacidades y configuración para esa
  vertical (`pack_retail`, `pack_farmacia`, …).
- **Capacidad**: la autoridad real de negocio (`catalogo_items`,
  `agenda_recursos`, `pagos`, `fidelizacion`, …). Cada servicio valida sus
  capacidades antes de operar; una capacidad inactiva devuelve
  `CAPACIDAD_NO_ACTIVA`.
- **Manifiesto**: `GET /api/v2/empresas/:id/features` devuelve qué puede hacer
  la empresa. El frontend muestra solo lo activo, sin hardcodear el rubro.
- **Configuración por empresa**: atributos JSON Schema, listas de precio,
  escalas, tarifas, ubicaciones, extensiones, tema, reglas de fidelización, etc.

## Principios de diseño

| Principio | Consecuencia |
|---|---|
| Multi-vertical por capacidades | No hay `farmacia-service` ni `dental-service`; hay servicios transversales y gating |
| Contratos primero | OpenAPI versionado por servicio; envelope `{data, meta}`; errores estructurados |
| Tenancy fuerte | RLS por empresa, roles y permisos; un tenant no ve datos de otro |
| Event-driven donde aporta | Outbox + RabbitMQ + DLQ; CQRS/Event Sourcing solo en `orders` |
| Migraciones aditivas | Nada se rompe al activar una capacidad o promover un pack |
| Configuración explícita | Una capacidad nueva se implementa una vez y luego se selecciona por pack/vertical/add-on |
| Fallar cerrado | Sin secreto, sin capacidad o sin tasa publicada, la operación se rechaza |

## Qué es y qué no es

- **Es** una plataforma para operar negocios reales de comercio, servicios y
  logística con un backend común.
- **No** es un ERP universal, un motor de workflow genérico ni un framework de
  bajo código.
- **No** crea un microservicio por rubro: los rubros se componen.
- **No** convierte por sí solo una regla de negocio nueva en configuración: si
  la capacidad no existe, primero se implementa (una vez) y después se vuelve
  seleccionable.

## Para quién

- Equipos que quieren lanzar **varios negocios o verticales** sin multiplicar
  el backend.
- Empresas que necesitan **multi-tenant**, roles, RLS y auditoría desde el día
  uno.
- Productos que crecerán por **capacidades** (pagos, fidelización, agenda,
  logística, analítica) y no por forks de código.

La decisión que rige la arquitectura es
[**ADR-0001 · Plataforma multi-vertical por capacidades**](docs/adr/0001-plataforma-multi-vertical.md)
(ADR-44 en el histórico de `docs/decisiones.md`). El documento
**Core Engine_Informe.docx v1.6** acota el dominio comercial original; el modelo
por capacidades lo generaliza a retail, farmacia, belleza, dental, flota,
talleres y verticales que se definan por configuración.

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

### Cómo se crea un negocio (sin desplegar código)

1. El usuario elige una **vertical** (`retail`, `farmacia`, `belleza`, `dental`,
   `flota`, o una creada por un admin).
2. Elige un **pack** de esa vertical (`pack_retail`, `pack_farmacia`, …).
3. El pack trae **capacidades activas**; el usuario puede sumar **add-ons**
   permitidos por la vertical.
4. Se crea la empresa con `POST /api/v2/empresas` indicando `vertical_id`,
   `pack_codigo` y, opcionalmente, `capacidades_extra`.
5. `GET /api/v2/empresas/:id/features` devuelve el **manifiesto**; el frontend
   muestra solo los módulos activos.
6. Un admin puede crear **verticales, modelos y packs** por API
   (`/api/v2/packs/...`) usando capacidades existentes.

```bash
# 1) consultar verticales y packs disponibles
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v2/packs/verticales
curl -H "Authorization: Bearer $TOKEN" "http://localhost:8080/api/v2/packs?vertical_id=retail"

# 2) crear la empresa con el pack y un add-on (ej. fidelización)
curl -X POST http://localhost:8080/api/v2/empresas \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"nombre":"TechFix","slug":"techfix","vertical_id":"retail",
       "pack_codigo":"pack_retail","pack_version":1,
       "capacidades_extra":["fidelizacion"]}'

# 3) verificar el manifiesto efectivo
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v2/empresas/<empresaId>/features
```

### Niveles de extensibilidad

| Nivel | ¿Requiere código? | Ejemplos |
|---|---|---|
| **Configuración pura** | No | Nuevo pack/vertical con capacidades existentes, atributos JSON Schema, listas de precio, extensiones, contratos, suscripciones, mesas |
| **Capacidad nueva en un servicio existente** | Sí, una vez | Fidelización dentro de `orders`/`finance` |
| **Servicio nuevo** | Sí + infraestructura | `loyalty-service` como bounded context propio |

La diferencia importa: **no todo es configuración**. Un pack nuevo con
capacidades existentes es configuración; una **tarjeta de fidelización** con
puntos, recompensas y canjes es una **capacidad nueva**. Se implementa una vez
(migración, dominio, endpoints, eventos, RLS y tests) y, a partir de ahí, sí se
vuelve seleccionable por configuración, igual que pagos, agenda o fulfillments.

### Qué es configuración pura y qué exige código

| Necesidad | Cómo se resuelve |
|---|---|
| Un nuevo rubro que usa catálogo, stock, agenda o rutas | Vertical + pack con capacidades existentes (sin código) |
| Campos extra por empresa (ficha técnica, expediente, etc.) | Extensiones versionadas + JSON Schema (sin código) |
| Precios por segmento, escalas, tarifas, promociones | Configuración de catálogo/pricing (sin código) |
| Reglas de operación (tipos de ítem, fulfillment, estados) | Registros configurables ya implementados (sin código) |
| Puntos, tarjetas, recompensas y canjes | **Capacidad nueva** `fidelizacion` (código una vez) |
| Pasarela de pago real, transportista externo, facturación electrónica | Integración nueva (código + credenciales) |
| Nómina, MRP, CRM con pipeline, workflow genérico | Fuera de alcance (no es un ERP) |

### Ejemplos de rubros

| Rubro | Vertical / modelo | Capacidades principales |
|---|---|---|
| Tienda, minimarket, ferretería, librería | `retail` / comercio | catálogo, stock, código de barras, delivery/pickup, facturación |
| Distribuidora / mayorista | `retail` / comercio | + listas de precio, escalas, ubicaciones, proveedores |
| Farmacia / botica | `farmacia` / comercio trazable | + lotes, vencimiento, FEFO |
| Salón, spa, barbería | `belleza` / servicios agendados | agenda, recursos, citas, facturación |
| Clínica / consultorio | `dental` / servicios agendados | agenda, historia clínica, recetas, registros |
| Flota / última milla | `flota` / logística | rutas, vehículos, field, cotizaciones, fulfillments |
| Restaurante / bar | `mixto` | mesas, comandas, catálogo, delivery/pickup |
| Taller / mantenimiento | `mixto` | agenda, servicios, repuestos, contratos |
| Alquiler de equipos | `mixto` | ítem alquiler, contratos/depósito, agenda |
| Membresías / cursos / digital | `mixto` | suscripciones, entrega digital, contratos |
| Venta a crédito | cualquiera | `ventas_credito` + finance |
| Fidelización | add-on | `fidelizacion` (tarjetas, puntos, recompensas, canjes) |

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
| `catalogo_items` | item | Productos, servicios y alquileres |
| `inventario_stock` | inventory | Stock, mínimos/máximos y movimientos |
| `inventario_lotes` | inventory | Lotes, vencimiento y consumo FEFO |
| `codigo_barras` | item | Etiquetas y escaneo en mostrador |
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
| `historia_clinica` | clinical | Registros clínicos por parte, validados por JSON Schema |
| `recetas` | clinical | Recetas por parte, validadas por JSON Schema |
| `registros` | records | Expedientes, mantenimiento y registros genéricos |
| `fulfillment_digital` | orders | Entrega digital con accesos, descargas y expiración |
| `contratos` | orders | Contratos con depósito, garantía y retención |
| `suscripciones` | orders | Compras recurrentes semanales/mensuales/anuales |
| `mesas` | orders | Mesas, comandas y estaciones de preparación |
| `ventas_credito` | finance | Cuentas de crédito y checkout con método `credito` |
| `fidelizacion` | loyalty | Tarjetas, puntos, recompensas y canjes (capacidad nueva) |

Los servicios **siempre activos** (`identity-service`, `api-gateway`,
`storage-service`, `notifications-service`) no se gatean. Los servicios de
feature validan `CapabilityGuard` antes de operar: una capacidad inactiva
devuelve el error estructurado `CAPACIDAD_NO_ACTIVA` y sus rutas no se anuncian
en el pack del tenant. El aislamiento de datos entre tenants es por **RLS**
(esquema por dominio + `empresa_id`).

### Fidelización (capacidad nueva)

La fidelización no es configuración pura: es una **capacidad nueva** que se
implementa una vez y luego se selecciona por pack, vertical o add-on.

**Modelo**

- `programa` por empresa: activo, puntos por unidad de moneda, expiración,
  niveles y reglas.
- `tarjeta` por empresa + cliente (`identity.partes`): código, puntos y estado.
- `movimientos`: acumulaciones, canjes, ajustes y expiraciones; con
  `order_id`/`event_id` para idempotencia.
- `recompensas`: catálogo de premios con costo en puntos y stock opcional.
- `canjes`: canje de una recompensa por una tarjeta, con estado y vínculo a la
  orden.

**Flujo**

1. El cliente compra y la orden pasa a `pagada`.
2. Se acumulan puntos de forma idempotente (una sola vez por orden).
3. El cliente consulta su tarjeta y su historial.
4. En checkout puede canjear una recompensa: se validan puntos, se descuentan y
   se aplica el beneficio.
5. La empresa administra programa, recompensas y ajustes; el cliente solo ve su
   tarjeta (RLS por empresa y parte).

**Cómo se selecciona al crear el negocio**

- La capacidad `fidelizacion` se ofrece como **add-on** en las verticales que la
  permiten.
- El alta de empresa acepta `capacidades_extra: ["fidelizacion"]` para activarla
  en la misma transacción.
- También puede activarse después con
  `PATCH /api/v2/empresas/:id/capacidades/fidelizacion`.
- El manifiesto (`GET /empresas/:id/features`) expone
  `fidelizacion.programas`, `fidelizacion.tarjetas` y `fidelizacion.canjes`; el
  frontend muestra el módulo solo si está activo.

> Estado: capacidad nueva; implementación en curso. Cuando esté publicada, se
> selecciona exactamente igual que pagos, agenda o fulfillments.

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
  identity  item     inventory  pricing  stores  orders  logistics  scheduling  commissions  finance  field  intelligence  storage  notifications
    :3001    :3015    :3013     :3014   :3003   :3004     :3005      :3011       :3006      :3007   :3008     :3009      :3010      :3012
  (commissions solo stage · market-intelligence en stage/prod)
       RabbitMQ · topic core-engine.events (+ DLQ, AD-04) + Postgres 16 + MinIO (un bucket por tenant, ADR-24)
```

Los bounded contexts se organizan **por capacidad de negocio**, no por rubro:
`identity` es la autoridad de tenants, roles, verticales, capacidades y packs;
`item`, `stores`, `orders`, `scheduling`, `logistics`, `finance`,
`commissions` y `field` habilitan las capacidades del pack; `storage`,
`notifications` y `intelligence` son transversales. No existen
`dentista-service`, `farmacia-service` ni `pulperia-service` (ADR-0001).

**Punto de entrada unico:** solo el servicio `gateway` publica un puerto de aplicación al host
(`8080:8080`); los **servicios** escuchan en `3001`–`3014` **solo dentro de la red interna
Docker `core-engine`** y no tienen mapeo de puertos al host. La excepción es **MinIO** (`9000`), que se
publica en **loopback** porque es el host local que Traefik firma como dominio HTTPS en las URLs
prefirmadas (ADR-24). Toda llamada externa pasa por `http://localhost:8080/api/v2/...` (o el dominio
del gateway en stage/produccion).

- **CQRS + Event Sourcing** solo en `orders` (alta tasa de escritura, cap. 3.2); el resto usa
  CRUD convencional sobre su propio esquema (baja tasa de escritura).
- **Outbox pattern** (ADR-03): cada servicio publica sus eventos en `x.outbox` en la misma
  transaccion; un reenviador los mueve al broker. RabbitMQ confirma publicacion (confirm channel).
- **Saga** (ADR-02): `order.created` → inventory descuenta stock (atomico) → `stock.reservado` →
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
docker compose -f docker-compose.dev.yml up -d --build   # app: Postgres + RabbitMQ + MinIO + Mailpit + gateway y 14 servicios (8080 host + datos/broker 5432/5672/15672 + minio 9000/9001 + mailpit 1025/8025)
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
| item-service | 3015 | No | `item` | `packages/item-service` (Rust, ADR-0018) |
| inventory-service | 3013 | No | `inventory` | `packages/inventory-service` (Rust, ADR-0018) |
| pricing-service | 3014 | No | `pricing` | `packages/pricing-service` (Rust, ADR-0018) |
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
| catálogo | `/catalog/items/**` (productos, servicios, alquileres, variantes, imágenes, componentes, series), `/catalog/combos`, `/catalog/importar`, `/catalog/escanear` |
| precios | `/catalog/listas-precio/**`, `/catalog/items/{id}/{precio,escalas,tarifas,cotizacion}`, `/catalog/promociones` |
| inventario | `/admin/inventario/**`, `/admin/ubicaciones/**`, `/catalog/inventario/movimientos` |
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
| operación | `GET /health` (liveness), `GET /ready` (readiness agregada: 11 servicios en producción, 12 en stage), `GET /metrics`, Swagger en `/docs` y `/docs-json` |

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

Cobertura actual: **426/426 operaciones documentadas** en los 14 servicios con
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
