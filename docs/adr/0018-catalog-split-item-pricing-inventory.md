# ADR-0018: Separación de `catalog-service` en `item-service`, `pricing-service` e `inventory-service`

- **Fecha**: 2026-11-15
- **Estado**: Aceptado — migración en curso (no se promueve hasta cerrar todos
  los PRs y el smoke de `staging`)
- **Ámbito**: `catalog-service` (items, precios, inventario, stock, operaciones
  y consumidor de la saga), gateway, esquemas PostgreSQL, CI/CD, compose y
  contratos OpenAPI
- **Referencias**: [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0003](./0003-notificaciones-correo.md),
  [ADR-0016](./0016-catalog-service-en-rust.md),
  [ADR-0017](./0017-identity-service-en-rust.md)

## Contexto

`catalog-service` es el servicio más grande del repositorio: ~18.300 líneas
Rust, 29 tablas, 74 operaciones OpenAPI y 116 pruebas. Nació como un catálogo
de productos, pero hoy concentra cuatro responsabilidades distintas:

1. **Catálogo maestro**: `items` unificados (producto, servicio y alquiler),
   variantes, atributos JSON Schema, imágenes, componentes/BOM, series,
   combos y el legado `productos`/`servicios`.
2. **Precios**: precio base, listas por empresa/segmento, escalas por cantidad,
   tarifas por vigencia, cotizaciones, histórico y promociones.
3. **Inventario**: stock por item y empresa, lotes FEFO, ubicaciones,
   movimientos, ajustes, reservas, transferencias y configuración de costeo.
4. **Operaciones**: importación masiva, escaneo por código de barras y
   utilidades de captura.

El acoplamiento principal está en los datos: `catalog.items` contiene
`precio_cents`, `costo_cents`, `track_inventory` y `stock`; los triggers de
`productos`/`servicios` sincronizan hacia `items`; y el consumidor de la saga
(`order.created`, `devolucion.solicitada`) reserva stock y escribe el Outbox en
la misma transacción. Además, `orders`, `stores` y `commissions` ya consumen
las APIs internas `/internal/items/**` y `/internal/inventario/**`, que son el
punto natural de corte.

La plataforma quiere ser una **fábrica de negocios agnóstica**: el vertical no
debe ser dueño del catálogo, del precio ni del stock. Estos tres son
capacidades transversales que cualquier vertical compone. Mantenerlas en un
solo proceso impide escalarlas, desplegarlas y evolucionarlas por separado.

## Decisión

Separar `catalog-service` en tres servicios con esquemas y ciclos de vida
propios:

| Servicio | Bounded context | Responsabilidad |
|---|---|---|
| **`item-service`** | Catálogo maestro / PIM | Qué se vende: identidad, tipo, atributos, variantes, imágenes, componentes, series, combos y ciclo de vida del ítem |
| **`pricing-service`** | Motor de precios | A cuánto se vende: precio base, listas, escalas, tarifas, cotizaciones, histórico y promociones |
| **`inventory-service`** | Inventario y almacenes | Cuánto hay y dónde: stock, lotes FEFO, ubicaciones, movimientos, ajustes, reservas y transferencias |

Se descarta el nombre `product-service` porque el agregado raíz ya es `items`
y el tipo incluye `producto`, `servicio` y `alquiler`. `product-service`
volvería a sesgar la plataforma hacia un solo vertical y al legado
`productos`.

### 1. Propiedad de datos

| Dominio | Tablas propias | Notas |
|---|---|---|
| `item-service` | `item.items`, `item.item_atributo_esquemas`, `item.item_imagenes`, `item.item_componentes`, `item.item_series`, `item.historico_precios`, `item.outbox` | `items` conserva el precio base, los umbrales `stock_minimo`/`stock_maximo` y `track_inventory`/`track_serial`. `stock` y `precio_cents` son proyecciones del contrato de items |
| `pricing-service` | `pricing.listas_precio`, `pricing.lista_precio_items`, `pricing.item_tarifas`, `pricing.promociones`, `pricing.outbox` | Resuelve listas, escalas, tarifas, cotizaciones y promociones. `lista_precio_items` con `lista_id NULL` es la escala base |
| `inventory-service` | `inventory.ubicaciones`, `inventory.item_stock_ubicacion`, `inventory.lotes_inventario`, `inventory.movimientos_inventario`, `inventory.ajustes_stock`, `inventory.reservas_lotes`, `inventory.reservas_ordenes`, `inventory.configuracion_inventario`, `inventory.proveedores`, `inventory.outbox` | `item_stock_ubicacion` es la fuente de las cantidades; `item.items.stock` es la proyección total mantenida por triggers |

Reglas:

- Cada servicio es dueño de escritura de su `SCHEMA` (R-DB-1). Las lecturas
  cruzadas son read-models explícitos de la transición: `inventory` y
  `pricing` leen `item.items`; `item` valida la ubicación de una serie contra
  `inventory.ubicaciones`. No hay escrituras cruzadas salvo las proyecciones
  acordadas (`item.items.stock` por triggers de inventario y
  `item.items.tipo_precio` por pricing).
- Hay dos FKs entre esquemas como red de seguridad del read-model:
  `stores.ofertas.item_id → item.items` y
  `item.item_series.ubicacion_id → inventory.ubicaciones`. El resto de
  referencias usa `item_id` lógico (UUID/texto) sin FK.
- `item_id` es el identificador global del ítem vendible. `pricing-service` e
  `inventory-service` lo referencian como texto/UUID.
- `item-service` es dueño del precio base y de su histórico
  (`item.historico_precios`). `pricing-service` no lo reescribe: solo resuelve
  listas/tarifas y marca `tipo_precio`.
- El stock inicial se captura al crear el item; a partir de ahí todas las
  variaciones las escribe `inventory-service` y `PATCH` con `stock` se rechaza.
- `costo_cents` se queda como dato de costeo en item/inventario. `proveedores`
  vive en inventario; si el vertical necesita compras y órdenes de compra, se
  evalúa un `procurement-service` posterior.
- Contrato v2 pendiente: retirar `stock` y `precio_cents` del payload de items
  exige resolverlos por API interna o versionar el contrato.

### 2. Contratos e integración

- Las rutas públicas se conservan durante la migración. El gateway enruta por
  prefijo al servicio dueño sin cambiar el contrato visible:
  - `/api/v2/catalog/items/**` → `item-service`
  - `/api/v2/catalog/listas-precio/**`, `/precio`, `/escalas`, `/tarifas`,
    `/promociones/**` → `pricing-service`
  - `/api/v2/admin/inventario/**`, `/api/v2/admin/ubicaciones/**`,
    `/api/v2/catalog/inventario/**` → `inventory-service`
- Las rutas internas se reparten igual:
  - `/internal/v2/items/**` → `item-service` (metadata sin precio/stock; los
    precios en lote y la cotización viven en `pricing-service`).
  - `/internal/inventario/**` → `inventory-service`.
- `orders`, `stores` y `commissions` siguen usando el mismo contrato; solo
  cambia la URL base del cliente o el gateway interno.
- Eventos nuevos, con Outbox y DLQ:
  - `item.created`, `item.updated`, `item.deleted`, `item.variant.changed`
  - `price.changed`, `price_list.updated`, `promotion.started/ended`
  - `stock.updated`, `stock.reserved`, `stock.failed`, `stock.released`,
    `stock.replenished`, `lot.expiring`
- `inventory-service` consume `order.created` y `devolucion.solicitada` y
  mantiene la reserva atómica y la idempotencia por `event_id`.
- Las mutaciones críticas siguen siendo asíncronas (R-AR-2). Las consultas de
  lectura entre servicios usan API interna con timeout y degradación.

### 3. Invariantes que no cambian

| Área | Invariante |
|---|---|
| Superficie | mismas rutas, métodos, códigos y envelope `{data, meta}`; errores `{codigo, mensaje, detalles?}` |
| Auth | mismos roles, permisos y capacidades; `x-internal-key` en rutas internas |
| Multi-tenancy | `empresa_id`/`tenant_id` y RLS con `request.jwt.claims` en cada servicio |
| Dinero | enteros en centavos; JSON con dos decimales; sin floats |
| Stock | reserva total o rechazo total; FEFO; descuento y Outbox en una transacción |
| Eventos | mismos nombres y payloads de la saga; `catalog.eventos_procesados` se sustituye por idempotencia propia de inventario |
| Observabilidad | `/health`, `/metrics`, `/docs`, `/docs-json`, logs JSON y `x-request-id` |
| Runtime | fail-fast de `DATABASE_URL`/`RABBITMQ_URL`; cierre ordenado; un puerto por servicio |

### 4. Estrategia de migración (strangler)

No se hace un big bang. Se extrae por el borde de menor acoplamiento y se
mantiene el contrato observable:

1. **Inventario primero.** Es el límite más claro, el de mayor concurrencia y
   el que ya tiene consumidor y endpoints internos. Se crea
   `inventory-service`, se mueven tablas, rutas, consumidor y pruebas; el
   `catalog-service` deja de escribir stock.
2. **Precios segundo.** Se extraen listas, escalas, tarifas, histórico y
   promociones; se mueven `precio_cents`/`costo_cents` y se publica
   `price.changed`.
3. **Items al final.** El catálogo restante se recorta a PIM y se renombra a
   `item-service`. Se retiran los triggers legacy y las rutas que ya no le
   pertenecen.
4. **Limpieza.** Se retira `catalog-service` del compose, CI, gateway y docs;
   se actualizan clientes internos y se cierra el ADR-0016 como absorbido.

Durante la transición se permiten **vistas de compatibilidad** y un flag de
enrutamiento en el gateway, pero no doble escritura permanente ni transacciones
distribuidas. Cada PR debe dejar el sistema arrancable y con la suite del
ámbito en verde.

## Plan de PRs

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR, índice y runbook de migración | `rules:check` verde; decisión registrada |
| 2 | Andamiaje de `inventory-service`: crate, config, DB/RLS, envelope, auth, métricas, Outbox/Rabbit, `/health`, `/metrics`, `/docs`, Dockerfile, CI y README | `cargo fmt`, `clippy`, `test` y gate de cobertura verdes |
| 3a | Núcleo de inventario: configuración, proveedores, niveles de stock, resumen y lotes FEFO | Pruebas PostgreSQL y HTTP verdes |
| 3b | Ubicaciones, movimientos, ajustes y transferencias | Pruebas PostgreSQL y HTTP verdes |
| 4 | Consumidor de saga en inventario: reserva, rechazo, devolución, idempotencia y Outbox | Pruebas de concurrencia y saga verdes |
| 5 | Migración de datos y rutas: esquema `inventory`, migración SQL, manifiesto, RLS, gateway y clientes internos | Integración PostgreSQL y contrato verde; `catalog` deja de servir inventario |
| 6 | Andamiaje y dominio de `pricing-service` (listas, escalas, tarifas, histórico, promociones, `price.changed`) | Suite y contrato verdes |
| 7 | Migración de precios: mover columnas, rutas y clientes; `item-service` deja de escribir precios | Integración y contrato verdes |
| 8 | Recorte de `catalog-service` a `item-service`: renombrar crate, retirar stock/precio/legacy, actualizar compose/CI/docs | Build, gates y OpenAPI verdes |
| 9 | Cierre: eliminar `catalog-service`, actualizar `SERVICES`/`RUST_SERVICES`, compose stage/prod, imágenes GHCR y runbooks | `npm run build`, `rules:check`, `baseline:guard`, `swagger:export` verdes |
| 10 | Verificación de staging: smoke completo, saga order→stock→pago→comisión→asiento, rollback documentado | Smoke de `staging` verde y evidencia en el PR de promoción |

Cada PR se abre contra la rama de integración de la migración
(`feat/catalog-split`) y **no se promueve a `develop`/`staging` hasta que toda
la cadena esté verde**. El PR final de promoción incluye la evidencia de los
gates y del smoke de `staging`.

## Verificación

1. `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
   `cargo test` y `cargo llvm-cov` por servicio.
2. Pruebas PostgreSQL de cada dominio con esquema efímero y RLS.
3. Pruebas de contrato: OpenAPI sin diff incompatible, envelope, errores,
   rutas internas y cabeceras de auth.
4. Pruebas de saga: `order.created` → reserva/rechazo → `stock.reservado`/
   `stock.fallido`; devolución → `stock.reintegrado`; idempotencia y DLQ.
5. `npm run build`, `npm run rules:check`, `npm run baseline:guard`,
   `npm run db:migrations`, `npm run db:rls` y `npm run swagger:export`.
6. Smoke local (`npm run demo`) y smoke de `staging` antes de promover.

## Consecuencias

### Positivas

- Tres capacidades reutilizables por cualquier vertical: PIM, pricing e
  inventario.
- Inventario escala y despliega por separado; el stock deja de compartir
  proceso con el catálogo.
- Precios evoluciona sin tocar el ciclo de vida del ítem.
- Se reduce el riesgo del servicio más grande y con menor cobertura del repo.

### Costes y riesgos

- La migración de datos es delicada: `items` hoy mezcla precio, costo y stock.
- Hay triggers legacy de `productos`/`servicios` que deben retirarse sin
  romper consumidores.
- `orders`, `stores` y `commissions` dependen de APIs internas de catálogo;
  hay que actualizar clientes y contratos en el mismo PR de corte.
- La cobertura actual de catálogo es 41,8 %: antes de mover lógica hay que
  añadir pruebas de contrato e integración.
- Durante la transición conviven los servicios; el gateway debe enrutar por
  prefijo y el rollback debe estar documentado.
- `81_item_schema` copia `catalog.items` a `item.items` y `83_item_fk_externas`
  repunta las claves foráneas externas (hoy `stores.ofertas.item_id`): con el
  corte, las ofertas se validan contra `item.items` y la tabla vieja queda solo
  para rollback.

## Criterios de aceptación

- `item-service`, `pricing-service` e `inventory-service` compilan, pasan sus
  gates y sirven el contrato actual sin cambios incompatibles.
- La saga de stock conserva atomicidad, idempotencia y Outbox.
- `catalog-service` desaparece del compose, CI, gateway y documentación.
- `npm run build`, `rules:check`, `baseline:guard`, `db:migrations`, `db:rls`
  y `swagger:export` en verde.
- Smoke de `staging` verde antes de la promoción a producción.

## Estado de implementación (2026-11-15)

- ADR aceptado; rama de migración `feat/catalog-split` creada desde `develop`.
- PR 1 completo: ADR, índice y runbook con la matriz de pruebas y el rollback.
- PR 2 completo: andamiaje de `inventory-service` (config fail-fast, PostgreSQL
  con RLS, envelope, auth, errores, logs, métricas, `/health`, `/metrics`,
  `/docs` y `/docs-json`). Sin rutas de dominio, consumidor ni RabbitMQ
  todavía; el servicio no se registra en `SERVICES` hasta el PR de corte para
  no desplegar una imagen vacía en `staging`.
- PR 3a completo: configuración de inventario, proveedores, niveles de
  stock, resumen y lotes FEFO con consumo atómico.
- PR 3b completo: ubicaciones, stock por ubicación, transferencias,
  movimientos, ajustes y superficie interna. Las tablas siguen en el esquema
  `catalog` como fuente transitoria; el PR 5 las mueve a `inventory`.
- PR 4 completo: consumidor `inventory.pedidos` con reserva atómica,
  rechazo total, FEFO, series, expansión BOM, devolución, idempotencia por
  `event_id`, Outbox transaccional y DLQ. El evento se publica con los mismos
  nombres (`stock.reservado`, `stock.fallido`, `stock.reintegrado`,
  `stock.updated`).
- PR 5 completo: esquema `inventory` con las tablas de inventario, `inventory.outbox`,
  copia idempotente desde `catalog`, RLS y grants; `inventory-service` registrado
  en `SERVICES`/CI/compose; gateway con destino propio y readiness 10/11;
  consumidor viejo retirado de `catalog-service`. `catalog.items.stock` queda
  como proyección transitoria hasta la extracción de `item-service` (PR 8).
- 69 pruebas unitarias + 7 de PostgreSQL de inventario + 9 de saga + 4 de
  ubicaciones y movimientos + 1 HTTP integral; cobertura medida 80,97 % líneas
  / 77,04 % funciones. Migración aplicada íntegra sobre PostgreSQL/PostGIS
  efímero.
- E2E local `npm run demo` 39/39 tras aplicar `75_inventory_schema` sobre la
  base de desarrollo y corregir la RLS del poller (`outbox_app`).
- PR 8 completo: `item-service` con esquema `item`, migración `81_item_schema`,
  gateway, compose, CI, release/promote/stage-images y OpenAPI. `catalog-service`
  queda retirado del repo; precios e inventario ya viven en sus servicios.
- PR 6-7 completos: `pricing-service` con listas, precios, escalas, tarifas,
  cotizaciones y promociones; esquema `pricing`, migración `79_pricing_schema`,
  gateway, compose, CI, release/promote y OpenAPI. `catalog.items.precio_cents`
  queda como precio base transitorio hasta `item-service`.
- PR 9 completo: limpieza de `item-service`. Se retiran los módulos duplicados
  de precios, tarifas, inventario y ubicaciones, sus handlers y el consumidor
  de pedidos; se restauran las rutas PIM de series y componentes que se habían
  perdido en la extracción; el esquema de pruebas queda reducido a las tablas
  propias del servicio. `PATCH /catalog/items/{id}` con `stock` responde
  `STOCK_GESTIONADO_POR_INVENTARIO`.
- Validación en `staging` (2026-11-16): PR de promoción #719, fix de FK
  `83_item_fk_externas` (#721/#723) y deploy stage con `npm run demo` 39/39 y
  gate de logs sin errores. `item-service:stage` publicado desde `staging`.
- PR 10 completo: `inventory.item_stock_ubicacion` es la fuente del stock.
  La migración `84_inventory_stock_owner` mueve los triggers de alta y delta de
  `catalog.items` a `item.items` (escribiendo en `inventory.*`), repara el
  desglose contra el total y garantiza una ubicación predeterminada por
  empresa. Movimientos, consumo de lotes y ajustes por ubicación publican
  `stock.updated` para refrescar las ofertas (RN-02).
- PR 11 completo: retiro definitivo del esquema `catalog` con la migración
  `85_retiro_catalog`. Antes de borrar se validan dependencias externas
  (FKs/vistas) y se conservan `item`, `pricing` e `inventory` como únicos
  esquemas del dominio. Con esto se cierra la ventana de rollback por tablas
  antiguas: volver atrás requiere restaurar un backup previo a `85`.
- PR 12 completo: contrato y propiedad de datos alineados. El OpenAPI de
  `item-service` deja de documentar `stock` como editable en `PATCH` y
  describe `STOCK_GESTIONADO_POR_INVENTARIO`; se retira el DTO duplicado de
  `pricing-service` que provocaba la colisión del contrato agregado. La
  matriz de propiedad de datos refleja el estado real (precio base e histórico
  en item, stock por ubicación en inventario) y deja explícito el contrato v2.
- PR 13 completo: contrato interno v2 de items ([ADR-0020](./0020-contrato-v2-items.md)).
  `/internal/v2/items/**` devuelve metadata sin precio/stock/series;
  `stores-service` resuelve precio en `pricing-service` y stock en
  `inventory-service`; `orders-service` compone metadata v2 con el batch de
  stock de inventario. El contrato público mantiene `precio`/`stock` como
  campos deprecados hasta migrar el frontend.
- `stock` y `precio_cents` siguen en `item.items` como proyección del contrato
  de items: `inventory-service` mantiene el stock por trigger y los clientes
  leen ambos campos en `GET /catalog/items`. Retirarlos exige versionar el
  contrato o resolver precio/stock por llamada interna; requiere su propio
  ADR.
