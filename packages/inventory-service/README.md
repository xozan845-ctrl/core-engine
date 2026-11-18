# inventory-service · stock, lotes y ubicaciones

`inventory-service` es la **autoridad única del stock físico**. Posee los
niveles por ubicación, los lotes y su vencimiento (consumo **FEFO**), los
movimientos (entrada, salida, ajuste, traslado), los proveedores, la
configuración de inventario y las **reservas de la saga de órdenes**.

Nace de la división de `catalog-service`
([ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)). El
catálogo maestro es de `item-service`; el precio es de `pricing-service`.

- [ADR-0018 · split del catálogo](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)
- [ADR-0020 · contrato v2 de items](../../docs/adr/0020-contrato-v2-items.md)
- Contrato: [`docs/openapi/inventory-service/`](../../docs/openapi/inventory-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `inventory-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest` |
| Puerto | `3013` (`PORT`) |
| Esquema | `inventory` (lee `item` para series/componentes) |
| Outbox | `inventory.outbox` |
| Capacidades | `inventario_stock`, `inventario_lotes` |
| Consume | `order.created`, `devolucion.solicitada` |
| Publica | `stock.reservado`, `stock.fallido`, `stock.reintegrado`, `stock.updated` |
| Dependencias | `identity` (capacidades), `item` (metadatos/series) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor + outbox |
| `OUTBOX_TABLA` | No | `inventory.outbox` | Outbox |
| `PORT` | No | `3013` | Puerto |
| `SERVICIO` | No | `inventory-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Rutas internas |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Capacidades |
| `ITEM_SERVICE_URL` | No | `http://item-service:3015` | Items/series |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `inventario/mod.rs` | Configuración, niveles, proveedores, lotes y consumo FEFO |
| `movimientos/mod.rs` | Entradas, salidas, ajustes y traslados (máx. 200 líneas) |
| `ubicaciones/mod.rs` | Multi-bodega/sucursal, stock por ubicación, transferencias |
| `reservas.rs` | Reserva/reintegro atómico de la saga, lotes y series |
| `stock.rs` | Helpers de stock total y delta sobre `item_stock_ubicacion` |
| `consumidor.rs` | Consumidor de `order.created` y `devolucion.solicitada` |
| `http/` | Router, validación, envelope, capacidades |
| `outbox.rs` / `rabbit.rs` | Publicación y consumo con DLQ |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `inventory.item_stock_ubicacion` | **Fuente única del stock**: `(empresa_id, item_id, ubicacion_id)` → `stock` |
| `inventory.ubicaciones` | Bodegas/sucursales/almacenes; una `predeterminada` por empresa |
| `inventory.lotes_inventario` | `codigo_lote`, `cantidad`, `cantidad_disponible`, `vence_en`, `estado` |
| `inventory.movimientos_inventario` | Cabecera + `lineas` JSONB, tipo, motivo, referencia, origen/destino |
| `inventory.ajustes_stock` | Ajustes con motivo y actor |
| `inventory.proveedores` | Proveedores del tenant |
| `inventory.configuracion_inventario` | `metodo_costeo` (`promedio`/`fifo`/`ueps`), `permitir_stock_negativo`, alertas, unidad por defecto |
| `inventory.reservas_ordenes` | Reserva por orden (idempotencia de la saga) |
| `inventory.reservas_lotes` | Detalle lote→orden/cantidad |
| `inventory.eventos_procesados` | Idempotencia por `event_id` |

### Invariantes

- El stock **nunca** vive en `item.items`: vive en `item_stock_ubicacion`. Un
  item con `track_inventory=false` no admite stock (`INVENTARIO_NO_APLICA`).
- Una empresa puede tener una ubicación **predeterminada**. Las entradas usan
  la predeterminada o, si no existe, la primera activa; si no hay ninguna, el
  alta de **stock inicial** crea `principal` y las operaciones de delta
  rechazan con `SIN_UBICACION`. Si el checkout envía `ubicacion_id`, la
  reserva y el descuento se hacen **solo** en esa ubicación (ADR-0024).
- `stock >= 0` salvo que la configuración permita negativos; aun así la saga de
  checkout exige disponibilidad real.
- Los lotes vencidos **no** se consumen.
- El consumo FEFO ordena por `vence_en NULLS LAST, creado_en, id`.
- Una transferencia entre ubicaciones **no altera el total** de la empresa.
- `fijar_stock` ajusta el desglose y registra la diferencia como movimiento.
- Las reservas descuentan **stock, lotes y series** en la misma transacción.

---

## 4. Reglas de negocio

| Regla | Comportamiento |
|---|---|
| RN-03 | Descuento atómico por línea; si una línea no alcanza, **se revierte todo** con `SAVEPOINT` y se emite `stock.fallido` |
| RN-06 | `devolucion.solicitada` reintegra stock, lotes y series, y emite `stock.reintegrado` |
| FEFO | Primero vence primero; lotes vencidos excluidos; si no alcanza → `STOCK_LOTE_INSUFICIENTE` |
| Seriales | Si el item `track_serial`, se reservan unidades `disponible` concretas; faltante → fallo de línea |
| Cotizaciones | `order.created` con `tipo_orden=cotizacion` solo marca el evento como procesado, no reserva |
| Entrega digital | No compromete stock físico |
| Componentes | Un item compuesto expande sus componentes antes de reservar |
| Movimientos | Tipos `entrada`, `salida`, `ajuste`, `traslado`; máx. 200 líneas; motivo/referencia obligatorios según tipo |
| Stock visible | Toda entrada/salida/ajuste emite `stock.updated` para que `stores` actualice ofertas (RN-02) |
| Costeo | `promedio`, `fifo`, `ueps`; valor inválido → `METODO_COSTEO_INVALIDO` |
| Lotes | `vence_en` pasado se considera vencido; no se permite consumir |
| Stock inicial | Fija el stock del item en la ubicación (no suma), exige producto con `track_inventory` y emite `stock.updated` con `event_id` determinista por item |

### Saga de stock (resumen)

```
order.created
  ├─ expandir componentes
  ├─ SAVEPOINT reserva_stock
  ├─ por línea: lock de stock, FEFO de lotes, series disponibles
  ├─ si TODO alcanza:
  │    UPDATE stock + lotes + series
  │    INSERT reservas_ordenes / reservas_lotes
  │    outbox: stock.reservado
  │    marca eventos_procesados
  └─ si algo falla:
       ROLLBACK TO SAVEPOINT
       outbox: stock.fallido (detalle por línea)
       marca eventos_procesados
```

Stock, lotes, series, outbox e idempotencia comparten **una sola transacción**.
Si el proceso muere antes del commit, no hay efecto parcial; si muere después,
el reintento ve `event_id` en `eventos_procesados` y no duplica.

---

## 5. API pública

Requiere capacidad `inventario_stock`; los lotes exigen además
`inventario_lotes`.

| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/v2/admin/inventario` | Niveles de stock (filtros empresa/negocio/ubicación) |
| POST | `/api/v2/admin/inventario/proveedores` | Crear proveedor |
| GET/POST | `/api/v2/admin/inventario/lotes` | Listar/crear lote |
| POST | `/api/v2/admin/inventario/lotes/consumir` | Consumir lotes en orden FEFO |
| GET/PUT | `/api/v2/catalog/configuracion-inventario` | Leer/guardar configuración |
| GET/POST | `/api/v2/admin/ubicaciones` | Listar/crear ubicación |
| PATCH | `/api/v2/admin/ubicaciones/{id}` | Actualizar ubicación |
| POST | `/api/v2/admin/ubicaciones/transferencias` | Transferir stock entre ubicaciones |
| GET | `/api/v2/admin/ubicaciones/items/{item_id}/stock` | Stock de un item por ubicación |
| GET | `/api/v2/admin/ubicaciones/{id}/stock` | Stock de una ubicación |
| PUT | `/api/v2/admin/ubicaciones/{id}/stock/{item_id}` | Fijar stock de un item en una ubicación |
| GET/POST | `/api/v2/catalog/inventario/movimientos` | Listar/crear movimiento |

Roles: administración y empresario (gateway) + permisos de inventario en el
servicio.

---

## 6. API interna (`x-internal-key`)

| Método | Ruta | Uso |
|---|---|---|
| POST | `/internal/inventario/stock` | Stock y series disponibles por lote de SKUs (orders/checkout) |
| POST | `/internal/inventario/stock/inicial` | Registrar stock inicial de un item (item-service) |
| GET | `/internal/inventario/resumen` | Resumen para reportes/commissions |
| GET | `/internal/inventario/lineas` | Líneas de inventario para reportes |

Respuestas sin envelope; errores con el mismo `{codigo,mensaje,detalles?}`.

---

## 7. Eventos

### Consume

| Evento | Cola | Efecto |
|---|---|---|
| `order.created` | `inventory.pedidos` | Reserva stock/lotes/series o emite `stock.fallido` |
| `devolucion.solicitada` | `inventory.pedidos` | Reintegra stock y series, emite `stock.reintegrado` |

### Publica

| Evento | Cuándo | Consumidores |
|---|---|---|
| `stock.reservado` | Reserva exitosa | orders (pasa a `pagada`) |
| `stock.fallido` | Falta stock/serie | orders (cancela), notifications |
| `stock.reintegrado` | Devolución | — |
| `stock.updated` | Movimiento/ajuste | stores (stock visible), intelligence |

Idempotencia: `inventory.eventos_procesados(event_id)`. Reintentos con backoff;
al agotar `REINTENTOS_MAXIMOS` → DLQ. La cola se declara con
`routing_keys = [order.created, devolucion.solicitada]`.

---

## 8. Seguridad y multi-tenancy

- Todas las consultas filtran por `empresa_id`; las ubicaciones y movimientos
  respetan `negocio_id` cuando existe.
- `EMPRESA_NO_AUTORIZADA` ante intento de operar otro tenant.
- Los roles de inventario se validan en el servicio (no solo en el gateway).
- Las rutas internas exigen `x-internal-key` en tiempo constante.
- El stock no se expone en el contrato público de items; solo a través de
  endpoints de inventario o de la proyección de `item-service`.

---

## 9. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas de outbox (pendientes, publicados, fallos) y HTTP comunes.
- Logs JSON con `x-request-id`; los eventos usan
  `inventory-event-<event_id>`.
- Worker de outbox cada 1 s; supervisor de RabbitMQ y reintento cada 10 s.
- Apagado ordenado: aborta outbox, cierra Rabbit y pool.

---

## 10. Pruebas

```bash
cd packages/inventory-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/inventario_pg.rs`: niveles, proveedores, lotes, FEFO.
- `tests/ubicaciones_pg.rs`: multi-bodega, transferencias, stock por ubicación.
- `tests/http_ubicaciones.rs`: contrato HTTP y validación.
- `tests/saga_pg.rs`: reserva, fallo atómico, reintegro e idempotencia.
- `tests/common/` + `tests/schema.sql`: provisión del esquema real.

---

## 11. Referencias

- [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)
- [ADR-0020](../../docs/adr/0020-contrato-v2-items.md)
- [Contrato público](../../docs/openapi/inventory-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/inventory-service/internal.openapi.json)
- [README de paquetes](../README.md)

- `POST /internal/inventario/devolucion` (servicio a servicio): restock parcial
  de una RMA, idempotente por `devolucion_id`; reintegra stock por ubicación,
  series vendidas y lotes reservados.
