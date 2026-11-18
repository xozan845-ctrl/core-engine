# orders-service · pedidos, CQRS y Event Sourcing

`orders-service` es el **bounded context de pedidos** y el único servicio de la
plataforma que usa **CQRS + Event Sourcing**. El historial append-only
`orders.eventos` es la fuente de verdad; las proyecciones `orden_vista`,
`orden_timeline` y `comisiones_vista` se mantienen por trigger para lectura.

Además del checkout, el servicio concentra: carrito, métodos y registros de
pago, ciclo de vida configurable, entregas digitales, contratos con
depósito/garantía, suscripciones recurrentes, salón (mesas/comandas) y la
capacidad de **fidelización**.

- [ADR-0013 · orders-service en Rust](../../docs/adr/0013-orders-service-en-rust.md)
- [ADR-0019 · fidelización por capacidades](../../docs/adr/0019-fidelizacion-por-capacidades.md)
- [ADR-0020 · contrato v2 de items](../../docs/adr/0020-contrato-v2-items.md)
- Contrato: [`docs/openapi/orders-service/`](../../docs/openapi/orders-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `orders-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest` |
| Puerto | `3004` (`PORT`) |
| Esquema | `orders` (lee `identity.negocios`/`partes`) |
| Outbox | `orders.outbox` |
| Capacidades | `cotizaciones`, `fulfillment_delivery`, `fulfillment_pickup`, `fulfillment_onsite`, `fulfillment_digital`, `contratos`, `suscripciones`, `mesas`, `fidelizacion`, `ventas_credito` (vía finance) |
| Consume | `stock.reservado`, `stock.fallido`, `stock.reintegrado` (cola `orders.stock`) |
| Publica | `order.created`, `order.status.updated`, `payment.procesado`, `devolucion.solicitada` |
| Dependencias | `stores`, `item`, `pricing`, `inventory`, `identity`, `scheduling`, `finance` |
| Workers | suscripciones cada 1 h; expiración de puntos cada 6 h; outbox cada 1 s |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor + outbox |
| `OUTBOX_TABLA` | No | `orders.outbox` | Outbox |
| `PORT` | No | `3004` | Puerto |
| `SERVICIO` | No | `orders-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Internos + clientes |
| `COMMISSION_RATE` | No | `0.12` | Comisión RN-04 |
| `PAGO_WEBHOOK_SECRET` | Recomendada | — | Webhook de pagos (`x-webhook-secret`) |
| `PAGO_PROVEEDOR` | No | `simulado` | Adaptador de pagos (`simulado`/`http`) |
| `PAGO_PROVEEDOR_URL` | Solo `http` | — | Base de la pasarela (`/cargos`, `/reembolsos`, `/contracargos`) |
| `PAGO_PROVEEDOR_KEY` | No | — | Bearer de la pasarela |
| `STORES_SERVICE_URL` | No | `http://stores-service:3003` | Ofertas |
| `ITEM_SERVICE_URL` | No | `http://item-service:3015` | Items |
| `PRICING_SERVICE_URL` | No | `http://pricing-service:3014` | Precios |
| `INVENTORY_SERVICE_URL` | No | `http://inventory-service:3013` | Stock |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Capacidades/partes |
| `SCHEDULING_SERVICE_URL` | No | `http://scheduling-service:3011` | Disponibilidad |
| `FINANCE_SERVICE_URL` | No | `http://finance-service:3007` | Crédito |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `event_store.rs` | Append-only, versión optimista por agregado, reintentos `23505` |
| `views.rs` | Proyección `orden_vista` y timeline |
| `checkout.rs` | Creación de orden, validaciones, transiciones, replay y comisión |
| `order_state.rs` | Estados, tipos, fulfillments y matriz de transiciones |
| `carrito.rs` | Carrito 30 min, máximo 99 por línea, enriquecido en vivo |
| `pagos.rs` | Métodos configurables, pagos, confirmación/rechazo, webhook |
| `tipos_orden.rs` | Tipos y transiciones configurables en BD |
| `fulfillments.rs` | Catálogo de fulfillments activos |
| `entregas.rs` | Accesos digitales por token, descargas y expiración |
| `contratos.rs` | Contratos, depósito, garantía, retención y movimientos |
| `suscripciones.rs` | Compras recurrentes y renovación atómica |
| `salon.rs` | Mesas, comandas y estaciones |
| `fidelizacion.rs` | Programa, tarjetas, movimientos, recompensas y canjes |
| `consumer.rs` | Saga de stock |
| `http/` | Routers por área, envelope y validación |
| `clients.rs` | Puertos HTTP a stores/item/pricing/inventory/identity/scheduling/finance |

### CQRS + Event Sourcing

```
Comando (POST /orders)
  → validaciones + carga de ofertas/stock/precios
  → append en orders.eventos (versión optimista)
  → outbox order.created
  → trigger actualiza orders.orden_vista / orden_timeline
Consulta (GET /orders/:id)
  → lee orden_vista (proyección)
Replay (POST /internal/orders/:id/reproyectar)
  → reconstruye desde orders.eventos y reescribe la proyección
```

- `aggregate_id` = orden, `aggregate_type` = `order`.
- La versión se incrementa por agregado; si hay conflicto se reintenta hasta 3
  veces con `SAVEPOINT` cuando participa de una transacción mayor.
- La proyección nunca es fuente de verdad: se puede reproyectar.

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `orders.eventos` | Event store append-only (`event_id`, `aggregate_id`, tipo, payload, metadata, version) |
| `orders.orden_vista` | Proyección de lectura de la orden |
| `orders.orden_timeline` | Timeline legible |
| `orders.comisiones_vista` | Proyección para comisiones |
| `orders.carritos` | Carrito por comprador, `expira_en` |
| `orders.metodos_pago` | Catálogo global (código, requiere confirmación, activo, config) |
| `orders.pagos` | Intentos/confirmaciones por orden |
| `orders.estados_orden`, `tipos_orden`, `transiciones_orden` | Ciclo de vida configurable |
| `orders.fulfillments` | Catálogo de fulfillments |
| `orders.entregas_digitales` | Token, estado, descargas, máximo, expiración |
| `orders.contratos`, `contrato_eventos` | Depósito/garantía/retención |
| `orders.suscripciones`, `suscripcion_ejecuciones` | Recurrencia |
| `orders.mesas`, `comandas` | Salón |
| `orders.fidelizacion_programas/tarjetas/movimientos/recompensas/canjes` | Fidelización |
| `orders.eventos_procesados` | Idempotencia de la saga |

---

## 4. Ciclo de vida de la orden

**Estados**: `creada`, `pagada`, `en_preparacion`, `enviada`, `aceptada`,
`rechazada`, `entregada`, `cancelada`, `devuelta`.

**Tipos**: `compra`, `reserva`, `cotizacion`, `orden_servicio`.

**Fulfillments**: `delivery`, `pickup`, `onsite`, `digital`, `none`.

| Tipo | Transiciones |
|---|---|
| `compra` | `creada→pagada/cancelada`; `pagada→en_preparacion/cancelada/devuelta`; `en_preparacion→enviada/cancelada`; `enviada→entregada/devuelta`; `entregada→devuelta` |
| `reserva` | `creada→pagada/cancelada`; `pagada→entregada/cancelada/devuelta`; `entregada→devuelta` |
| `cotizacion` | `creada→enviada/cancelada`; `enviada→aceptada/rechazada/cancelada` |
| `orden_servicio` | `creada→pagada/cancelada`; `pagada→en_preparacion/cancelada/devuelta`; `en_preparacion→entregada/cancelada`; `entregada→devuelta` |

`aceptada`, `rechazada`, `cancelada` y `devuelta` son terminales. Un salto
inválido devuelve `TRANSICION_INVALIDA`. La matriz efectiva se consulta en
`orders.transiciones_orden`, por lo que un admin puede añadir aristas sin
desplegar (los tipos base están sembrados).

---

## 5. Checkout (`POST /api/v2/orders`)

Validaciones en orden:

1. `metodo_pago` existe y está activo; si es `credito`, exige
   `cliente_parte_id` y que todos los items sean de una sola empresa.
2. Fulfillment permitido y capacidad activa:
   `delivery`→`fulfillment_delivery`, `pickup`→`fulfillment_pickup`,
   `onsite`→`fulfillment_onsite`, `digital`→`fulfillment_digital`;
   `cotizacion`→`cotizaciones`; `credito`→`ventas_credito`.
3. Carrito no vacío ni expirado (RN-05).
4. Cada línea: oferta activa, cantidad 1–99, stock suficiente. Si la línea
   trae `ubicacion_id`, el stock se valida contra esa bodega/sucursal
   (ADR-0024); sin ella se usa el total de la empresa.
5. `reserva`: exige `cliente_parte_id`, `recurso_id`, `inicia_en`, `termina_en`,
   cantidad 1 y fechas ISO; verifica disponibilidad en scheduling.
6. Precio: base + margen de la oferta; comisión `total × COMMISSION_RATE`
   (RN-04; `0` desactiva la comisión). Las promociones vigentes de pricing se
   aplican como descuento por línea sobre el precio final (ADR-0025). Si la
   orden trae `cupon_codigo`, pricing valida y canjea el cupón sobre el total
   (ADR-0028). El modelo `venta_directa` del pack desactiva la comisión por
   línea y permite `vendedor_id` nulo (ADR-0023). En cotización la comisión es
   0 hasta aceptar.
7. Crédito: registra la venta en finance antes de confirmar; si falla, se
   anula el crédito y no se crea la orden.
8. Todo se persiste en una transacción: evento + outbox + proyección.

La orden nace `creada`; inventory consume `order.created` y responde
`stock.reservado` (→ `pagada`) o `stock.fallido` (→ `cancelada`).

---

## 6. Carrito (RN-05)

- Un carrito por comprador; **expira a los 30 minutos**.
- No reserva stock.
- Cantidad por línea: **1–99**; cantidades repetidas se agrupan.
- Se enriquece en vivo contra `stores` (precio, stock, estado de la oferta).
- Operaciones: ver, vaciar, agregar, actualizar cantidad (0 elimina), quitar.

---

## 7. Pagos

### Métodos

Catálogo global administrado por `super_admin`/`admin`:
`GET /api/v2/orders/metodos-pago` (autenticado),
`POST /api/v2/admin/metodos-pago`,
`PATCH /api/v2/admin/metodos-pago/{codigo}`.

Campos: `codigo`, `nombre`, `requiere_confirmacion`, `activo`, `config`.

- Si `requiere_confirmacion=true` (efectivo/transferencia), `stock.reservado`
  deja la orden en `creada` hasta confirmación manual.
- Si es automático/simulado, pasa a `pagada` y emite `payment.procesado`.
- `credito` usa el cupo de finance y emite el mismo flujo.

### Confirmación manual

`GET /api/v2/admin/pagos?order_id=…`,
`POST /api/v2/admin/pagos/{id}/confirmar`,
`POST /api/v2/admin/pagos/{id}/rechazar`.

### Proveedor, reembolsos y contracargos

- `PAGO_PROVEEDOR=simulado|http` elige el adaptador (ACL `ProveedorPagos`).
  `http` usa `PAGO_PROVEEDOR_URL` y `PAGO_PROVEEDOR_KEY` (Bearer).
- Los métodos con `config.cargo_externo=true` (o `simulado`) crean un cargo y
  guardan `referencia` + `proveedor`.
- `POST /api/v2/admin/pagos/{id}/reembolsar` body `{monto_cents, motivo?}`:
  reembolso total o parcial, acumula `monto_reembolsado_cents`.
- `POST /api/v2/admin/pagos/{id}/contracargo` body `{monto_cents, motivo?}`:
  marca `contracargo`; no mueve la orden por sí solo.
- `POST /api/v2/admin/pagos/{id}/conciliar`: marca `conciliado_en`.
- Cada operación deja un evento en `orders.pagos_eventos`.

### Webhook

`POST /internal/pagos/webhook` con cabecera `x-webhook-secret` igual a
`PAGO_WEBHOOK_SECRET`. Acepta `{pago_id, estado: confirmado|rechazado, actor?}`.
Si el secreto no está configurado, **todo webhook se rechaza** (fallar cerrado).
No pasa por el gateway.

---

## 8. Cotizaciones

| Método | Ruta | Actor |
|---|---|---|
| POST | `/orders/{id}/cotizacion/enviar` | vendedor/empresario/admin |
| POST | `/orders/{id}/cotizacion/aceptar` | comprador |
| POST | `/orders/{id}/cotizacion/rechazar` | comprador |

Aceptar una cotización genera la orden de compra y dispara el flujo normal.

---

## 9. Entregas digitales

- Se generan al **crear la orden** para cada línea `fulfillment=digital` (token,
  máximo de descargas y expiración desde los atributos de la oferta).
- La descarga solo se habilita cuando la orden está en `pagada`, `enviada` o
  `entregada`.
- `GET /api/v2/pedidos/{id}/entregas-digitales` lista accesos (dueño/admin).
- `GET /api/v2/entregas-digitales/{token}` consume un acceso público:
  valida estado de la orden (`pagada`/`enviada`/`entregada`), expiración,
  máximo de descargas y estado del acceso.
- Errores: `ENTREGA_EXPIRADA`, `ENTREGA_REVOCADA`, `ENTREGA_SIN_DESCARGAS`,
  `ENTREGA_DIGITAL_NO_CONFIGURADA`.

---

## 10. Contratos

- Tipos: `alquiler`, `servicio`, `compra`, `otro`.
- Estados: `borrador`, `activo`, `finalizado`, `cancelado`.
- Al activar con depósito > 0 se registra `deposito_recibido`.
- `POST /contratos/{id}/finalizar` recibe `retenido` (≤ depósito) y `motivo`;
  registra `retencion` y `deposito_devuelto` en `contrato_eventos`.
- `GET /contratos/{id}/movimientos` expone el historial.
- Un contrato finalizado no se reabre.

---

## 11. Suscripciones

- Intervalos: `semanal`, `mensual`, `anual`.
- Estados: `activa`, `pausada`, `cancelada`; transiciones
  `activa→pausada/cancelada`, `pausada→activa/cancelada`.
- Cada vencimiento crea una **orden normal** con precio y stock vigentes.
- La renovación es atómica (reclamo por `proxima_ejecucion`) para que dos
  workers no dupliquen.
- `POST /suscripciones/{id}/renovar` fuerza una renovación inmediata.
- Worker cada **1 hora** renueva las vencidas.
- `GET /suscripciones/{id}/ejecuciones` lista el historial.

---

## 12. Salón: mesas y comandas

- Mesas: `libre`, `ocupada`, `reservada`, `inactiva`; capacidad, zona, código.
- Comandas: `pendiente`, `en_preparacion`, `lista`, `entregada`, `cancelada`;
  `estacion` libre (cocina, bar, …), `items_json`, notas.
- Una comanda puede nacer de una orden o de items libres; sin items devuelve
  `COMANDA_SIN_ITEMS`.
- Capacidad `mesas` obligatoria.

---

## 13. Fidelización (capacidad `fidelizacion`)

- **Programa único por empresa**: activo, `puntos_por_unidad` (0–100 000),
  `expira_meses`, `config`.
- **Tarjeta por parte** (`identity.partes`) con código `FID-XXXXXXXX`, saldo y
  estado (`activa`, `bloqueada`, `cerrada`).
- **Movimientos**: `acumulacion`, `canje`, `ajuste`, `expiracion`, con
  `saldo_tras` y `order_id`.
- **Acumulación**: `puntos = total_cents × puntos_por_unidad / 100` (entero).
  Solo si la orden pasa a `pagada` y hay `cliente_parte_id` con tarjeta activa.
  Idempotente por índice único `(tarjeta_id, order_id)`.
- **Recompensas**: catálogo con costo en puntos y stock opcional.
- **Canjes**: validan saldo y stock y descuentan ambos en una transacción;
  pueden vincularse a una orden.
- **Expiración**: worker cada **6 h**; tarjetas sin movimientos durante
  `expira_meses` generan movimiento `expiracion` idempotente.
- El comprador ve solo sus tarjetas (`mis-tarjetas`, `mis-canjes`).

---

## 14. API pública (resumen)

> Fuente de verdad: `docs/openapi/orders-service/public.openapi.json`.

| Área | Rutas |
|---|---|
| Órdenes | `POST/GET /orders`; `GET /orders/{id}`, `/{id}/timeline`; `GET /orders/{admin/todas,vendedor/mis-ventas,tienda/{id},empresa,empresa/resumen}` |
| Cotización | `POST /orders/{id}/cotizacion/{enviar,aceptar,rechazar}` |
| Carrito | `GET/DELETE /carrito`; `POST /carrito/items`; `PATCH/DELETE /carrito/items/{ofertaId}` |
| Pagos | `GET /orders/metodos-pago`; `POST/PATCH /admin/metodos-pago`; `GET /admin/pagos`; `POST /admin/pagos/{id}/{confirmar,rechazar,reembolsar,contracargo,conciliar}` |
| Tipos/fulfillment | `GET/POST/PATCH /admin/tipos-orden`; `POST/DELETE .../transiciones`; `GET /orders/fulfillments`; `POST/PATCH /admin/fulfillments` |
| Entregas | `GET /pedidos/{id}/entregas-digitales`; `GET /entregas-digitales/{token}` |
| Contratos | `GET/POST /contratos`; `GET/PATCH /contratos/{id}`; `POST /contratos/{id}/finalizar`; `GET /contratos/{id}/movimientos` |
| Suscripciones | `GET/POST /suscripciones`; `GET /suscripciones/{id}`; `POST /{id}/{pausar,reanudar,cancelar,renovar}`; `GET /{id}/ejecuciones` |
| Salón | `GET/POST/PATCH /mesas`; `GET/POST/PATCH /comandas` |
| Fidelización | `GET/PUT /fidelizacion/programa`; `GET/POST /fidelizacion/tarjetas`; `GET /tarjetas/{id}`, `/{id}/movimientos`; `POST /tarjetas/{id}/ajustes`; `GET/POST /recompensas`; `PATCH /recompensas/{id}`; `POST /canjes`; `GET /mis-tarjetas`, `/{id}/movimientos`, `/mis-recompensas`; `POST /mis-canjes` |

---

## 15. API interna (`x-internal-key`)

| Método | Ruta | Consumidor |
|---|---|---|
| GET | `/internal/orders?…` | finance, commissions, analytics |
| GET | `/internal/orders/{id}` | finance, commissions |
| POST | `/internal/orders/{id}/transicion?estado=…` | logistics |
| POST | `/internal/orders/{id}/reproyectar` | operación/replay |
| GET | `/internal/orders/{id}/historia` | auditoría |
| POST | `/internal/pagos/webhook` | proveedor de pagos (secreto propio) |

---

## 16. Eventos

### Consume (`orders.stock`)

| Evento | Efecto |
|---|---|
| `stock.reservado` | Si la orden está `creada` y el pago no requiere confirmación → `pagada` + `payment.procesado` |
| `stock.fallido` | `creada → cancelada` (RN-03) |
| `stock.reintegrado` | Registro de devolución (informativo) |

Idempotencia por `event_id` en `orders.eventos_procesados`.

### Publica

| Evento | Cuándo |
|---|---|
| `order.created` | Alta de orden/cotización/reserva |
| `order.status.updated` | Toda transición |
| `payment.procesado` | Pago confirmado (automático, manual o webhook) |
| `devolucion.solicitada` | Devolución aceptada |

---

## 17. Seguridad y multi-tenancy

- Cada consulta filtra por `empresa_id`; el comprador solo ve sus órdenes.
- El vendedor ve las de su tienda; el empresario las de su empresa; admin
  todas. En venta directa (`vendedor_id` nulo) la orden se ve por empresa y
  comprador, no por vendedor.
- `empresa_id`/`negocio_id` se congelan en la orden al crearla.
- La capacidad se valida contra `identity` por empresa **y negocio**.
- El webhook usa un secreto propio y falla cerrado si no está configurado.
- Los tokens de entrega digital son opacos y de un solo recurso.

---

## 18. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas HTTP + outbox; logs con `x-request-id`.
- Workers: outbox 1 s, suscripciones 1 h, expiración de puntos 6 h.
- Supervisor RabbitMQ, reintento cada 10 s, DLQ.
- Apagado ordenado: aborta workers, cierra Rabbit y pool.

---

## 19. Pruebas

```bash
cd packages/orders-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/pedidos_pg.rs` (~2300 líneas): checkout, transiciones, saga,
  proyecciones, replay, contratos, suscripciones, salón y fidelización contra
  PostgreSQL real.
- `tests/http_pedidos.rs`: contrato HTTP, roles y validación.
- Tests unitarios de `order_state` (matriz), `money`, carrito y fidelización.

---

## 20. Referencias

- [ADR-0013](../../docs/adr/0013-orders-service-en-rust.md)
- [ADR-0019](../../docs/adr/0019-fidelizacion-por-capacidades.md)
- [Runbook de fidelización](../../docs/runbook-fidelizacion.md)
- [Contrato público](../../docs/openapi/orders-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/orders-service/internal.openapi.json)
- [README de paquetes](../README.md)
