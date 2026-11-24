# packages/ · microservicios y API Gateway de Core Engine

Este directorio contiene **todos los runtimes** de Core Engine: quince
microservicios Rust y el **API Gateway**. Cada subdirectorio es un *crate*
autónomo con su `Cargo.toml`, `Dockerfile`, `README.md`, `src/` y `tests/`.
No existe un paquete compartido de runtime: la duplicación de
`config/db/errors/rabbit/outbox/metrics` es una decisión consciente para
preservar la autonomía del *bounded context* (R-AR-5, R-MS-1).

La plataforma es **multi-vertical por capacidades**: el rubro no es código, es
configuración (vertical + pack versionado + capacidades + datos). Los servicios
son transversales a los rubros; no hay `farmacia-service` ni `dental-service`.

- Decisión marco: [ADR-0001](../docs/adr/0001-plataforma-multi-vertical.md)
- Reglas de arquitectura: [R-AR](../docs/rules/architecture/01-arquitectura.md)
- Reglas de microservicios: [R-MS](../docs/rules/microservice/01-estructura.md)
- Reglas de gateway: [R-GW](../docs/rules/gateway/01-auth.md)

---

## 1. Mapa de paquetes

> **Fuente de verdad:** el catálogo se genera desde
> `packages/<servicio>/service.json` en
> [`docs/architecture/catalogo-servicios.md`](../docs/architecture/catalogo-servicios.md)
> (R-MS-9/R-AF-4). Esta tabla es la vista curada para humanos; si difiere del
> manifiesto, manda el manifiesto y `npm run architecture:check` falla.

| Paquete | Puerto | Esquema(s) | Entornos | Rol |
|---|---:|---|---|---|
| [`api-gateway`](./api-gateway/README.md) | **8080** | — | dev/stage/prod | Único punto de entrada, JWT, políticas, proxy, rate limit, readiness |
| [`identity-service`](./identity-service/README.md) | 3001 | `identity` | dev/stage/prod | Auth, sesiones, usuarios, empresas, negocios, personal, partes, capacidades, packs, extensiones, API keys, onboarding |
| [`item-service`](./item-service/README.md) | 3015 | `item` | dev/stage/prod | Catálogo maestro: productos, servicios, alquileres, variantes, componentes, series, combos, importación |
| [`inventory-service`](./inventory-service/README.md) | 3013 | `inventory` | dev/stage/prod | Stock, lotes FEFO, ubicaciones, movimientos, reservas y saga de stock |
| [`pricing-service`](./pricing-service/README.md) | 3014 | `pricing` | dev/stage/prod | Listas de precio, escalas, tarifas por fecha, cotizaciones y promociones |
| [`stores-service`](./stores-service/README.md) | 3003 | `stores` | dev/stage/prod | Tienda del vendedor, storefront, tema, contenido, ofertas y margen |
| [`orders-service`](./orders-service/README.md) | 3004 | `orders` | dev/stage/prod | CQRS + Event Sourcing, carrito, checkout, pagos, contratos, suscripciones, mesas/comandas, fidelización, entregas digitales |
| [`logistics-service`](./logistics-service/README.md) | 3005 | `logistics` | dev/stage/prod | Envíos, guías, flotas, asignación híbrida y transiciones logísticas |
| [`scheduling-service`](./scheduling-service/README.md) | 3011 | `scheduling` | dev/stage/prod | Recursos reservables, disponibilidad, bloqueos, citas y recordatorios |
| [`finance-service`](./finance-service/README.md) | 3007 | `finance` | dev/stage/prod | Comercio (bancos, créditos, gastos, compras, caja), contabilidad, tributación, facturación, proyecciones y KPIs |
| [`field-service`](./field-service/README.md) | 3008 | `field` | dev/stage/prod | Operación de campo: personal, clientes, vehículos, rutas, visitas, tracking, incidencias, cumplimiento y sync offline |
| [`storage-service`](./storage-service/README.md) | 3010 | `storage` | dev/stage/prod | Bucket por tenant, URLs prefirmadas, cuotas, media pública, MinIO/S3 |
| [`notifications-service`](./notifications-service/README.md) | 3012 | `notifications` | dev/stage/prod | Consumidor de eventos del pedido y correo transaccional (log/SMTP) |
| [`analytics-service`](./analytics-service/README.md) | 3016 | `analytics` | dev/stage/prod | Hechos de venta, cohortes, reportes programados, analítica de negocio/empresa/plataforma |
| [`commissions-service`](./commissions-service/README.md) | 3006 | `commissions` | **solo dev/stage** | Comisiones, liquidaciones quincenales, estado de cuenta de vendedor/empresa y reportes |
| [`market-intelligence-service`](./market-intelligence-service/README.md) | 3009 | `intelligence` | dev/stage/prod | Demanda, tendencias, forecast, anomalías, mapas de calor, scores y calidad de datos |

### Servicios siempre activos vs. servicios con capacidades

| Tipo | Servicios |
|---|---|
| **Siempre desplegados, sin gate de pack** | `identity`, `api-gateway`, `storage`, `notifications`, `analytics`* |
| **Gateados por capacidad** | `item`, `inventory`, `pricing`, `stores`, `orders`, `logistics`, `scheduling`, `finance`, `field`, `commissions`, `intelligence` |

\* `analytics` valida su propia capacidad `analitica_negocios`, pero esta se
siembra activa por defecto en verticales, packs y empresas.

Un servicio gateado consulta `identity` (`CapacidadesHttp`) antes de operar y
rechaza con `403 CAPACIDAD_NO_ACTIVA` cuando el tenant no tiene la capacidad.
El gateway **no** aplica ese gating: solo autentica y enruta.

---

## 2. Topología y flujo de una petición

```
                    ┌──────────────────────────────────────────┐
   Web / móvil ────▶│ api-gateway :8080                        │
                    │ TLS fuera · CORS · rate limit · JWT HS256 │
                    │ políticas por ruta/rol · proxy streaming  │
                    └───────────────┬──────────────────────────┘
                                    │ red Docker `core-engine`
     ┌──────────────┬───────────────┼───────────────┬──────────────┐
     ▼              ▼               ▼               ▼              ▼
 identity        item/inventory   orders         finance        storage
   :3001           :3015/:3013     :3004          :3007          :3010
     …               …               …               …              …
                                    │
                         RabbitMQ topic `core-engine.events`
                         (+ DLQ, reintentos, outbox por servicio)
                                    │
                    PostgreSQL 16 (un esquema por dominio)
                    MinIO (un bucket por tenant)
```

- Solo el gateway publica puerto al host. Los microservicios escuchan en
  `3001`–`3016` dentro de la red interna.
- En desarrollo se publican además Postgres (`5432`), RabbitMQ
  (`5672`/`15672`), MinIO (`9000`/`9001`) y Mailpit (`1025`/`8025`).
- En stage/producción solo se publican `gateway:8080` y `minio:9000` (loopback,
  host firmado de las URLs prefirmadas). La consola de MinIO nunca se publica.
- El gateway enruta las superficies admin de `notifications` y `analytics`; el
  resto de su operación es por eventos.

### Cabeceras de confianza interna

El gateway construye un conjunto nuevo de cabeceras (nunca reenvía las del
cliente):

| Cabecera | Origen |
|---|---|
| `x-user-id`, `x-user-email`, `x-user-rol`, `x-user-nombre` | Claims JWT |
| `x-tenant` | `tenant_id` |
| `x-negocio` | `negocio_id` |
| `x-user-personal` | `personal_id` |
| `x-user-participacion` | `participacion_porcentaje` |
| `x-user-rol-empresa` | `rol_empresa_id` |
| `x-user-permisos` | `permisos` (CSV) |
| `x-request-id` | Entrante o UUID v4 |
| `x-internal-key` | **Solo** en llamadas servicio→servicio, nunca desde el borde |

Los microservicios confían en `x-user-*` porque solo pueden recibirlas del
gateway o de otro servicio en la red interna. Las rutas `/internal/**` exigen
`x-internal-key` y no se publican por el gateway.

---

## 3. Modelo de tenancy y capacidades

La jerarquía real es:

```
empresa (tenant facturable, identity.empresas)
  ├── region_tributaria (jurisdicción efectiva, ADR-0049)
  └── negocio (unidad operativa, identity.negocios)
        ├── vertical_id + pack versionado
        ├── capacidades efectivas (identity.negocio_capacidades)
        └── datos operativos con empresa_id / negocio_id
```

- **Empresa**: titular fiscal y comercial. Agrupa uno o más negocios y elige
  la región tributaria (país/región) al aplicar el pack.
- **Negocio**: unidad donde ocurre la operación (sucursal, marca, línea). Tiene
  su propio vertical/pack/capacidades. El JWT lleva `negocio_id`.
- **Vertical**: modelo de operación (`retail`, `farmacia`, `belleza`, `dental`,
  `flota`, `mixto`, …).
- **Pack**: preset versionado de capacidades (`pack_retail` v1, …) y de las
  regiones tributarias que admite (`identity.pack_regiones`).
- **Región tributaria**: configuración fiscal del tenant (moneda, IVA, IR,
  documento, locale y zona horaria). No es una capacidad booleana: la elige el
  dueño al aplicar el pack y finance la usa como jurisdicción efectiva.
- **Capacidad**: autoridad de negocio (`catalogo_items`, `agenda_recursos`,
  `pagos`, `fidelizacion`, …).
- **Manifiesto**: `GET /api/v2/empresas/:id/features` informa al frontend qué
  puede mostrarse.

### Capacidades y servicio que las aplica

| Capacidad | Servicio | Habilita |
|---|---|---|
| `catalogo_items` | `item` (+ `pricing`, `stores`) | Productos, servicios, alquileres, variantes, combos |
| `codigo_barras` | `item` | Etiquetas y escaneo |
| `inventario_stock` | `inventory` | Stock, mínimos/máximos, movimientos |
| `inventario_lotes` | `inventory` | Lotes, vencimiento, FEFO |
| `agenda_recursos` | `scheduling` | Recursos, disponibilidad, citas, bloqueos |
| `fulfillment_delivery` | `logistics` | Entrega con flota de plataforma |
| `fulfillment_pickup` | `logistics` | Retiro en tienda |
| `fulfillment_onsite` | `logistics` | Servicio a domicilio |
| `logistica_rutas` | `logistics` | Flota propia, rutas y paradas |
| `cotizaciones` | `orders` | Presupuesto → aceptación → orden |
| `facturacion` | `finance` | Facturas, comprobantes y régimen fiscal de la región elegida |
| `liquidaciones` | `commissions` | Comisiones y cortes |
| `operacion_campo` | `field` | Personal, rutas, visitas, tracking |
| `inteligencia_mercado` | `intelligence` | Demanda, tendencias, forecast, score |
| `historia_clinica` / `recetas` / `registros` | `identity` (registros) | Expedientes validados por JSON Schema |
| `fulfillment_digital` | `orders` | Accesos, descargas y expiración |
| `contratos` | `orders` | Depósito, garantía, retención |
| `suscripciones` | `orders` | Compras recurrentes |
| `mesas` | `orders` | Mesas, comandas, estaciones |
| `ventas_credito` | `finance` (+ `orders`) | Cuentas de crédito y checkout con `credito` |
| `fidelizacion` | `orders` | Tarjetas, puntos, recompensas, canjes |

### Aislamiento de datos

- Un **esquema PostgreSQL por dominio**. Ningún servicio escribe en el esquema
  de otro.
- Las tablas operativas llevan `empresa_id` y, cuando aplica, `negocio_id`.
- **RLS** por `empresa_id` (y por negocio en tablas con `negocio_id`). El pool
  fija `request.jwt.claims` para que las políticas evalúen el tenant.
- Las lecturas cruzadas entre esquemas existen solo como *joins* de lectura en
  consultas de reporte (p. ej. `analytics` lee `identity.empresas`); nunca como
  escritura.
- Los servicios reciben el tenant por cabecera/JWT, no por parámetro de URL
  salvo en rutas administrativas explícitas.

---

## 4. Contrato HTTP común

### Envelope de éxito

Toda respuesta pública `2xx` con cuerpo JSON se envuelve:

```json
{
  "data": { "...": "..." },
  "meta": {
    "timestamp": "2026-11-18T15:04:05.123Z",
    "path": "/api/v2/orders?page=1",
    "method": "POST",
    "statusCode": 201
  }
}
```

No se envuelven: `/health`, `/metrics`, `/docs*`, `/internal/**`,
`/api/v2/storage/public/**`, respuestas `204`, errores ni binarios.

### Errores

```json
{ "codigo": "CAPACIDAD_NO_ACTIVA", "mensaje": "…", "detalles": { "…": "…" } }
```

- `detalles` es opcional y solo aporta contexto de dominio; nunca trazas.
- Códigos estables en MAYÚSCULAS (`NO_ENCONTRADO`, `CONFLICTO`,
  `VALIDACION`, `STOCK_INSUFICIENTE`, `TRANSICION_INVALIDA`, …).
- 400 validación/regla, 401 no autenticado, 403 sin permiso o capacidad,
  404 no existe, 409 conflicto de estado/stock, 413 cuerpo grande,
  422 no usado de forma sistemática, 500 interno, 502/504 solo en el borde.

### Convenciones de datos

- **Dinero**: enteros en **centavos** dentro de la lógica y la BD; el JSON
  expone strings con dos decimales (`"1150.00"`). Se aceptan `1150`,
  `"1150.00"` y `"1150,50"` en entradas; el parser rechaza más de dos
  decimales y valores negativos.
- **IDs**: UUID v4 (strings). Los slugs son otra clave pública con historial y
  `301` en cambios.
- **Fechas**: ISO-8601 UTC en el transporte; los cálculos de negocio usan
  `America/Managua` cuando la regla es local (cortes, declaraciones,
  recordatorios).
- **Paginación**: parámetros `page`/`limit` o `limite`/`offset` según el
  servicio; las respuestas incluyen metadatos de paginación dentro de `data`.
  No hay un contrato único rígido, sí un estilo consistente.
- **Idempotencia**: eventos por `event_id` en `<schema>.eventos_procesados`;
  operaciones sensibles (confirmar pago, canje, corte) validan estado antes de
  mutar.
- **Versionado**: superficie pública `/api/v2/**`; interna `/internal/**`. El
  contrato OpenAPI versionado es la fuente de verdad.

### Rutas internas

- Prefijo `/internal/`.
- Cabecera obligatoria `x-internal-key` (comparación en tiempo constante).
- No pasan por el gateway; se invocan por nombre de servicio
  (`http://<servicio>:<puerto>`).
- Devuelven JSON sin envelope y con el mismo formato de error.
- Tienen timeout configurable (`INTERNAL_REQUEST_TIMEOUT_MS`, 3 s por defecto).

---

## 5. Persistencia y migraciones

- PostgreSQL 16 (imagen PostGIS en dev para inteligencia geoespacial).
- Las migraciones viven en [`infra/db/init/`](../infra/db/init/) y se aplican
  en orden numérico. Son **aditivas**: activar una capacidad o promover un pack
  no rompe datos existentes.
- `scripts/config/db-migrations.json` es el manifiesto de hashes: una migración
  ya aplicada **no se edita** (R-DB-6).
- En Supabase (stage/prod) se aplica además
  [`infra/db/supabase/99_rls.sql`](../infra/db/supabase/99_rls.sql) para activar
  RLS por rol (`auth.rol()`, `auth.uid()`).
- Cada servicio declara su tabla de outbox con `OUTBOX_TABLA`
  (`orders.outbox`, `inventory.outbox`, …). Nunca se hardcodea.

### Esquemas por servicio

| Esquema | Dueño | Tablas principales |
|---|---|---|
| `identity` | identity | usuarios, sesiones, api_keys, empresas, negocios, regiones_tributarias, pack_regiones, empresa_region_historial, empresa_capacidades, negocio_capacidades, verticales, packs, pack_capacidades, modelos_operacion, extensiones, partes, contactos, direcciones, relaciones, registros, roles, puestos, asignaciones, invitaciones, auditoría, permisos |
| `item` | item | items, item_atributo_esquemas, item_imagenes, item_componentes, item_series, historico_precios |
| `inventory` | inventory | item_stock_ubicacion, lotes_inventario, movimientos_inventario, ajustes_stock, proveedores, ubicaciones, configuración, reservas_ordenes, reservas_lotes |
| `pricing` | pricing | listas_precio, lista_precio_items, item_tarifas, promociones, historico_precios_base |
| `stores` | stores | tiendas, tienda_slugs, ofertas, historico_precios, eventos_procesados |
| `orders` | orders | eventos (ES), orden_vista, orden_timeline, carritos, pagos, métodos_pago, tipos/estados/transiciones, fulfillments, entregas_digitales, contratos, contrato_eventos, suscripciones, ejecuciones, mesas, comandas, fidelización (5 tablas), eventos_procesados |
| `logistics` | logistics | envíos, asignaciones_envio, eventos_procesados |
| `scheduling` | scheduling | recursos, disponibilidad, bloqueos, citas, recordatorios, eventos_procesados |
| `finance` | finance | plan_cuentas, asientos, asiento_detalles, comprobantes, series (empresa/región), jurisdicciones, tasas_impuesto, regímenes, sujetos, declaraciones (tenant/plataforma), proyecciones, métricas, tipos_cambio, crédito_cuentas, workspace_registros |
| `field` | field | personal, clientes, vehículos, rutas, paradas, pedidos, asistencia, incidencias, tracking, visitas, cumplimiento |
| `storage` | storage | buckets, objetos |
| `notifications` | notifications | eventos_procesados, plantillas, webhooks, webhook_entregas |
| `analytics` | analytics | hechos_venta, comisiones, reportes_programados, reporte_ejecuciones, eventos_procesados |
| `commissions` | commissions | comisiones, pagos, liquidaciones, liquidaciones_empresas, compensaciones_devoluciones, eventos_procesados |
| `intelligence` | intelligence | hechos_venta, rendimiento_vendedor, cobertura_zona, métricas_producto, puntos_calor, tendencias, feature_store, invalid_events, vistas materializadas |

---

## 6. Eventos: Outbox, RabbitMQ y saga

### Patrón Outbox (R-AR-3)

Cada mutación que publica un evento escribe en `<schema>.outbox` **dentro de la
misma transacción** que el cambio de negocio. Un publicador en segundo plano
(intervalo de 1 s) lee pendientes, publica en RabbitMQ con *confirm channel* y
marca el registro. Si el broker cae, el evento no se pierde: queda en outbox.

Cada servicio tiene su propio `outbox.rs` (duplicado a propósito). La tabla se
toma de `OUTBOX_TABLA`.

### Topic y colas

- Exchange/topic: `core-engine.events`.
- Cada consumidor declara una cola con sus *routing keys*.
- Los mensajes llevan `tipo`, `event_id`, `data` y metadatos de correlación.
- El reintento usa backoff exponencial; al superar `REINTENTOS_MAXIMOS` el
  mensaje va a la DLQ. Los servicios exponen reinyección controlada donde
  aplica.
- La idempotencia se implementa con `event_id` en
  `<schema>.eventos_procesados` dentro de la misma transacción del efecto.

### Catálogo de eventos

| Evento | Publica | Consume |
|---|---|---|
| `order.created` | orders | inventory (saga), finance, notifications (sin correo), scheduling (reservas) |
| `stock.reservado` | inventory | orders |
| `stock.fallido` | inventory | orders, notifications |
| `stock.reintegrado` | inventory | — |
| `stock.updated` | inventory | stores, intelligence |
| `payment.procesado` | orders | logistics, finance, commissions, notifications |
| `order.completado` | orders | commissions, analytics, notifications |
| `order.status.updated` | orders | logistics, notifications |
| `shipment.started` | logistics | notifications |
| `devolucion.solicitada` | orders | inventory, finance, commissions, notifications |
| `comision.acreditada` | commissions | finance, intelligence, analytics |
| `declaracion.generada` | finance | — |
| `asiento.registrado` | finance | — |
| `comprobante.emitido` | finance | — |
| `appointment.created` / `appointment.cancelled` / `appointment.reminder.due` | scheduling | — |
| `venta.geolocalizada` | ingesta externa/manual | intelligence |
| `storage.objeto.subido` / `storage.objeto.eliminado` | storage | — |

> **No todos los servicios publican eventos.** `identity`, `item`, `pricing`,
> `stores`, `field` y `notifications` son hoy síncronos o consumidores puros;
> `analytics` e `intelligence` solo consumen. El outbox está implementado en
> varios crates aunque no siempre se inserten eventos.

> Los nombres de eventos son **descriptivos y en español** en el dominio
> (`pedido.creado`, `stock.reservado`); por compatibilidad histórica varios
> siguen en inglés (`order.created`, `payment.procesado`). No se renombran sin
> una migración de consumidores.

### Saga de checkout (ADR-02)

```
1. POST /api/v2/orders (orders)
   └─ crea orden en estado `creada` + evento `order.created` (outbox)
2. inventory consume `order.created`
   ├─ descuenta stock atómicamente por línea (RN-03)
   ├─ éxito  → `stock.reservado`
   └─ fallo  → `stock.fallido`
3. orders consume el resultado
   ├─ `stock.reservado` → estado `pagada`, emite `payment.procesado`
   │   (si el método requiere confirmación, espera confirmación manual)
   └─ `stock.fallido`   → estado `cancelada`
4. logistics consume `payment.procesado` → crea envío / guía
5. finance consume `payment.procesado`/`order.created` → asientos y comprobantes
6. commissions consume `order.completado`/`payment.procesado` → comisión
7. notifications consume todo el ciclo → correo
8. analytics/intelligence consolidan hechos
```

La orden sigue `creada → pagada → en_preparacion → enviada → entregada`, con
`cancelada` y `devuelta` como ramas. Las transiciones se validan en
`orders-service`; `logistics-service` puede avanzar estados vía
`PATCH /api/v2/orders/:id/estado` (el gateway enruta ese método a logistics).

---

## 7. Patrones internos de los crates

Todos los servicios comparten la misma estructura conceptual, aunque el código
esté duplicado:

| Archivo/módulo | Responsabilidad |
|---|---|
| `main.rs` | Bootstrap: config, pool, Rabbit, router, tareas, señales |
| `lib.rs` | Declaración de módulos y API para tests de integración |
| `config.rs` | Lectura de entorno y *fail-fast* (R-MS-5) |
| `db.rs` | Pool `sqlx`, transacciones, claims de RLS, helpers |
| `errors.rs` | `ApiError` + mapeo a HTTP `{codigo,mensaje,detalles?}` |
| `state.rs` | `AppState` con `Arc` de pool, outbox, clientes y puertos |
| `auth.rs` | Contexto de usuario, roles, capacidades, `x-internal-key` |
| `outbox.rs` | Inserción y publicación de eventos |
| `rabbit.rs` | Conexión, declaración de colas, consumo, reintento, DLQ |
| `logs.rs` | JSON estructurado con `x-request-id` por tarea |
| `metrics.rs` | Contadores/gauges/histograma Prometheus |
| `http/` | Router, handlers, envelope, validación; **sin SQL** |
| `<dominio>/` | Agregados, casos de uso y SQL explícito |
| `clients.rs` | Clientes HTTP internos con timeout y *puerto* testeable |
| `tests/` | Unitarias del crate + PostgreSQL real (`TEST_DATABASE_URL`) |

### Dependencias de runtime

`axum` 0.8, `tokio`, `sqlx` 0.8 (Postgres, rustls), `lapin` 2 (salvo donde no
hay eventos), `reqwest` (rustls), `serde`/`serde_json`, `chrono`/`chrono-tz`,
`uuid`, `thiserror`. Servicios con necesidades específicas añaden
`jsonwebtoken`/`bcrypt`/`lettre` (identity), `jsonschema` (identity, item),
`aws-sdk-s3` (storage), `unicode-normalization` (slugs).

No hay ORM: el SQL es explícito. No hay framework de DI: los puertos se
inyectan por `AppState` y `Arc<dyn Trait>`.

---

## 8. Observabilidad

### Endpoints de infraestructura por servicio

| Ruta | Contenido |
|---|---|
| `GET /health` | Liveness con `{servicio, estado/uptime}`. Sin auth. |
| `GET /ready` | Solo gateway (agrega `/health` de los 11/12 servicios) |
| `GET /metrics` | Prometheus text exposition |
| `GET /docs` | Swagger UI |
| `GET /docs-json` | OpenAPI del servicio (público + interno) |
| `GET /docs-internal-json` | Solo en servicios con contrato interno separado (p. ej. notifications) |

### Métricas comunes

| Métrica | Tipo | Etiquetas |
|---|---|---|
| `core_engine_http_requests_total` | counter | `estado`, `metodo`, `ruta`, `servicio` |
| `core_engine_http_server_errors_total` | counter | idem |
| `core_engine_http_relevant_client_errors_total` | counter | idem (401/403/409) |
| `core_engine_http_request_duration_seconds` | histogram | idem + `le` |
| `core_engine_outbox_published_events_total` | counter | por servicio |
| `core_engine_outbox_pending_events` | gauge | por servicio |
| `core_engine_<subsistema>_<nombre>_total` | counter | convención de negocio |
| `core_engine_<subsistema>_<nombre>_<unidad>` | gauge | convención de negocio |

Buckets de latencia: `0.05, 0.1, 0.2, 0.4, 0.8, 1.6, 3.2` s.

### Logs

JSON por línea: `nivel`, `ts`, `servicio`, `request_id`, `usuario_id`, `rol`,
`tenant_id`, `negocio_id` cuando existen. El `x-request-id` se propaga del
gateway al servicio y de ahí a los eventos (`*-event-<event_id>`).

### Capa opcional

`docker-compose.observability.yml` añade Prometheus y Grafana. Prometheus
scrapea `/metrics` de los servicios; Grafana provisiona el dashboard de
latencia p99, RPS, 5xx y backlog de RabbitMQ.

---

## 9. Configuración

Cada servicio valida al arrancar lo que necesita y **no arranca** si falta algo
crítico. Variables comunes:

| Variable | Usada por | Notas |
|---|---|---|
| `DATABASE_URL` | todos | Obligatoria; el servicio falla si no conecta |
| `RABBITMQ_URL` | todos menos gateway/field/identity (identity puede no consumir) | Obligatoria en los que publican/consumen |
| `OUTBOX_TABLA` | publicadores | `orders.outbox`, `inventory.outbox`, … |
| `JWT_SECRET` | identity, gateway | HS256, mismo valor |
| `JWT_ACCESS_TTL` / `JWT_REFRESH_TTL` | identity, gateway | 900 s / 7 d por defecto |
| `INTERNAL_API_KEY` | todos | Protege `/internal/**` |
| `INTERNAL_REQUEST_TIMEOUT_MS` | clientes internos | 3000 ms por defecto |
| `SERVICIO`, `PORT`, `TZ` | todos | Identidad del proceso |
| `*_SERVICE_URL` | según servicio | URLs internas (ver cada README) |
| `COMMISSION_RATE` | orders, commissions, finance | 12 % por defecto |
| `PAGO_WEBHOOK_SECRET` | orders | Firma del webhook de pagos |
| `EMAIL_MODE`, `EMAIL_FROM`, `SMTP_*` | identity, notifications | `log` o `smtp` |
| `MINIO_*`, `STORAGE_*` | storage | Endpoint, credenciales, cuotas, TTL |
| `PLATFORM_TENANT_ID` | identity, storage, stores | Tenant de assets de plataforma |
| `PLATFORM_LOGISTICS_TENANT_ID` | identity, logistics | Flota global |
| `EMITIR_COMPROBANTES_FISCALES` | finance | Activa emisión automática |
| `STORAGE_CONTENT_TYPES_PERMITIDOS` | storage | Whitelist de MIME |
| `TEMA_HERENCIA_TTL_MS` | stores | Caché de herencia de tema |

La matriz completa, con defaults y obligatoriedad, está en el README de cada
paquete. En stage/producción `RABBITMQ_PASSWORD` es obligatoria en el compose.

---

## 10. Desarrollo local

Requisitos: **Node >= 20** (tooling de gobernanza), **Rust estable** y
**Docker con Compose v2**.

```bash
# En la raíz del repo
npm install
cp .env.example .env
docker compose -f docker-compose.dev.yml up -d --build

# Verificación
curl -s http://localhost:8080/health
curl -s -H "x-internal-key: $INTERNAL_API_KEY" http://localhost:8080/ready

# Un servicio aislado
docker compose -f docker-compose.dev.yml up -d --build orders-service
cd packages/orders-service && cargo test
```

> En discos exFAT/FAT32 (sin symlinks) `npm install` falla con `EISDIR`. Usa
> ext4/APFS/NTFS para el checkout.

### Build de un crate

```bash
cd packages/<servicio>
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --summary-only
```

### Pruebas de integración

- Cada crate trae tests que provisionan su propio esquema contra PostgreSQL
  real (`TEST_DATABASE_URL`). No hay dobles de base de datos.
- La suite transversal vive en `scripts/tools/integration-runner.mjs`
  (`npm run test:integration`).
- La matriz multi-vertical (gating, packs, RLS por vertical, pilotos) se
  ejecuta con `npm run multi-vertical:check` y los E2E de `qa-harness/`.

---

## 11. Entrega y entornos

| Etapa | Rama | Dónde | Compose |
|---|---|---|---|
| Desarrollo | `develop` | Local | `docker-compose.dev.yml` |
| Stage | `staging` | Dockploy | `docker-compose.stage.yml` (incluye `commissions`) |
| Producción | `main` | Dockploy | `docker-compose.yml` (sin `commissions`) |

- `feature/*` → PR → `develop`; `develop` → PR → `staging`; `staging` → PR →
  `main`. A `main` solo se llega por promoción.
- Las imágenes se publican en GHCR (`ghcr.io/xozan845-ctrl/core-engine-<servicio>:<env>`).
- El gateway hornea el OpenAPI agregado: el gate G-8 falla si el contrato cambió
  sin regenerar (`npm run swagger:export`).
- Cobertura Rust con *ratchet* por servicio (R-COV-1).

---

## 12. Cómo contribuir

### Añadir una capacidad a un servicio existente

1. Crear migración aditiva en `infra/db/init/` y actualizar el manifiesto de
   hashes.
2. Registrar la capacidad en `identity` (seed/vertical/pack) y su gating.
3. Implementar el dominio + endpoints + `CapabilityGuard`.
4. Emitir/consumir eventos si la capacidad cruza contextos.
5. Añadir tests unitarios y de integración; actualizar OpenAPI con
   `npm run swagger:export`.
6. Actualizar `README.md` del servicio y este catálogo.

### Añadir un microservicio

La guía completa es [`docs/architecture/04-guia-nuevo-servicio.md`](../docs/architecture/04-guia-nuevo-servicio.md).
Resumen:

1. ADR que justifique el nuevo *bounded context*.
2. `packages/<nombre>-service/service.json` con perfil, puerto, esquema,
   capacidades, eventos y SLO (R-MS-9).
3. Crate en `packages/<nombre>-service` con la estructura R-MS-1/R-MS-10.
4. Esquema propio, migraciones, RLS y `eventos_procesados`/`outbox` si aplica.
5. Entrada en `docker-compose.dev.yml`/`.stage.yml`/`.yml`.
6. Destino y política en `api-gateway` (incluida `readiness.rs`).
7. Ruta interna en `identity` si necesita capacidades/tenants.
8. Documentación: README del paquete, catálogo, OpenAPI y reglas.
9. `npm run architecture:report` y `npm run rules:check` en verde. La matriz de
   CI y el catálogo se derivan del manifiesto; no se editan listas a mano.

### Checklists rápidas

- [ ] `cargo fmt`, `clippy -D warnings`, `cargo test` verdes.
- [ ] Migración aditiva y hasheada.
- [ ] Capacidad validada antes de operar.
- [ ] `empresa_id`/`negocio_id` en toda escritura.
- [ ] Eventos por outbox, nunca publicación directa en transacción.
- [ ] Consumidor idempotente por `event_id`.
- [ ] `/health`, `/ready`, `/metrics`, logs JSON y `x-request-id`.
- [ ] Errores con código estable y sin stacktrace.
- [ ] OpenAPI regenerado y README actualizado en el mismo PR.
- [ ] `service.json` y catálogo generado sin drift.

---

## 13. Mapa de documentación

| Documento | Contenido |
|---|---|
| [`../README.md`](../README.md) | Visión de producto, multi-vertical y arranque |
| [`../docs/architecture/`](../docs/architecture/) | Declaración arquitectónica: principios, contrato de servicio, fitness functions, catálogo y guía de servicio nuevo |
| [`../docs/rules/`](../docs/rules/) | Reglas ejecutables de arquitectura, CI, datos, docs, git, gateway, microservicios, observabilidad y tests |
| [`../docs/adr/`](../docs/adr/) | Decisiones de arquitectura (ADR-0001..0050) |
| [`../docs/api/core-engine.openapi.json`](../docs/api/core-engine.openapi.json) | Contrato público agregado |
| [`../docs/api/core-engine.internal.openapi.json`](../docs/api/core-engine.internal.openapi.json) | Contrato interno agregado |
| [`../docs/openapi/<servicio>/`](../docs/openapi/) | Contratos por servicio (público + interno) |
| [`../docs/runbook-multi-vertical.md`](../docs/runbook-multi-vertical.md) | Alta y migración de tenants/verticales |
| [`../docs/runbook-gateway-auth.md`](../docs/runbook-gateway-auth.md) | Diagnóstico de autenticación de borde |
| [`../docs/runbook-fidelizacion.md`](../docs/runbook-fidelizacion.md) | Operación de fidelización |
| [`../docs/runbook-observabilidad.md`](../docs/runbook-observabilidad.md) | Prometheus/Grafana |
| [`../infra/db/init/`](../infra/db/init/) | Esquema y migraciones |
| [`../qa-harness/`](../qa-harness/) | Suites E2E y matriz de reglas |

---

## 14. Glosario

| Término | Significado |
|---|---|
| **Tenant / empresa** | Titular facturable; frontera de aislamiento de datos |
| **Negocio** | Unidad operativa dentro de una empresa |
| **Vertical** | Modelo de operación de un rubro |
| **Pack** | Preset versionado de capacidades |
| **Capacidad** | Autoridad de negocio activable por tenant/negocio |
| **Manifiesto** | Respuesta que declara las features efectivas |
| **Outbox** | Tabla transaccional de eventos pendientes de publicar |
| **DLQ** | Cola de mensajes muertos tras agotar reintentos |
| **Saga** | Coreografía de eventos que mantiene consistencia entre servicios |
| **Envelope** | `{data, meta}` de las respuestas públicas |
| **RLS** | Row-Level Security de PostgreSQL |
| **ACL** | Capa anticorrupción para integraciones externas |
| **CQRS/ES** | Separación lectura/escritura y Event Sourcing (solo `orders`) |
| **FEFO** | First Expired, First Out (consumo de lotes por vencimiento) |
| **API key** | Credencial de máquina emitida por `identity`, canjeable por JWT corto |
