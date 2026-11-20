# logistics-service · envíos, flotas y ciclo logístico

`logistics-service` es el **bounded context de despacho**. Genera la guía por
orden y empresa, decide la flota (propia, de plataforma o híbrida), registra
asignaciones, avanza estados de envío y colabora con `orders` para mover el
ciclo de vida del pedido.

También expone las transiciones logísticas de la orden
(`PATCH /api/v2/orders/:id/estado`), que el gateway enruta **aquí**, no a
`orders`.

- [ADR-0010 · logistics-service en Rust](../../docs/adr/0010-logistics-service-en-rust.md)
- [ADR-0002 · envelope y desacoplamiento](../../docs/adr/0002-envelope-y-desacoplamiento.md)
- Contrato: [`docs/openapi/logistics-service/`](../../docs/openapi/logistics-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `logistics-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest` |
| Puerto | `3005` (`PORT`) |
| Esquema | `logistics` (lee `identity.negocios` en asignaciones) |
| Outbox | `logistics.outbox` |
| Capacidades | `fulfillment_delivery`, `logistica_rutas` |
| Consume | `payment.procesado`, `order.status.updated` (cola `logistics.pagos`) |
| Publica | `shipment.started`, `order.completado`, `devolucion.solicitada` |
| Dependencias | `orders` (consulta/transición), `identity` (capacidades) |
| Tenants especiales | `PLATFORM_LOGISTICS_TENANT_ID` (flota de plataforma) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor + outbox |
| `OUTBOX_TABLA` | No | `outbox` (en compose se define `logistics.outbox`) | Outbox |
| `PORT` | No | `3005` | Puerto |
| `SERVICIO` | No | `logistics-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Cliente orders + internos |
| `ORDERS_SERVICE_URL` | No | `http://orders-service:3004` | Órdenes |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Capacidades |
| `PLATFORM_LOGISTICS_TENANT_ID` | No | `00000000-0000-4000-8000-000000000001` | Flota global |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `envios/mod.rs` | Guía, proveedor, asignación, estados, transición de orden |
| `consumidor.rs` | `payment.procesado` / `order.status.updated`, idempotencia |
| `clients.rs` | Cliente interno de orders (consulta y avance) |
| `outbox.rs` | Eventos transaccionales |
| `http/envios.rs` | Handlers públicos |
| `auth.rs` | Contexto, capacidades, clave interna |
| `money.rs` | Formato de montos (`"1150.00"`) |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `logistics.envios` | `guia`, `order_id`, `empresa_id`, `negocio_id`, `parte_id`, `proveedor_tipo`, `proveedor_tenant_id`, `monto_cents`, `estado` |
| `logistics.asignaciones_envio` | Historial: proveedor anterior/nuevo, tenant, origen (`automatica`/`empresario`), actor |
| `logistics.eventos_procesados` | Idempotencia de eventos |
| `logistics.outbox` | Eventos pendientes |

### Invariantes

- **Una guía por `(order_id, empresa_id, negocio_id)`**: la generación es
  idempotente; si ya existe, se devuelve la misma y se registra la asignación
  automática si aplica.
- Formato de guía: `BGH-<base36(millis)>-<6 hex>`.
- `monto_cents` es el total de la orden (o la porción de la empresa) al momento
  de preparar el envío.
- Un envío con proveedor fijo **no** permite reasignación (`ASIGNACION_FIJA`);
  solo el modo híbrido deja elegir.
- Solo el tenant proveedor puede avanzar el estado de su envío.
- Estados de envío: `en_preparacion` (inicial), `enviada`, `entregada`,
  `cancelada`.
- La capacidad `logistica_pod` exige receptor y foto/firma antes de `entregada`.
  `POST /api/v2/envios/{id}/prueba-entrega` guarda la evidencia.
- Estados de orden que logística puede fijar: `en_preparacion`, `enviada`,
  `entregada`, `cancelada`, `devuelta`.

---

## 4. Selección de flota

La flota se deriva de las capacidades activas de la empresa/negocio:

| Capacidades | Proveedor |
|---|---|
| `logistica_rutas` + `fulfillment_delivery` | **Híbrido**: el empresario elige (`empresa` o `plataforma`) |
| Solo `logistica_rutas` | Flota **propia** (`proveedor_tipo=empresa`) |
| Solo `fulfillment_delivery` | Flota de **plataforma** (`proveedor_tipo=plataforma`) |

- La flota de plataforma vive en `PLATFORM_LOGISTICS_TENANT_ID`.
- La asignación automática no sobrescribe una existente
  (`ON CONFLICT DO NOTHING`).
- El empresario puede asignar en modo híbrido con
  `POST /api/v2/empresarios/me/envios/{id}/asignacion` indicando
  `{tipo: "empresa"|"plataforma"}`.

---

## 5. Flujo de despacho

```
payment.procesado
  → consultar orden en orders (items por empresa)
  → agrupar líneas por (empresa, negocio)
  → preparar un envío por grupo (guía idempotente)
  → elegir proveedor según capacidades
  → outbox: shipment.started

order.status.updated
  ├─ entregada → outbox: order.completado (comisiones devengan)
  └─ devuelta  → outbox: devolucion.solicitada

PATCH /envios/{id}/estado
  → actualizar envío
  → si todos los envíos de la orden están en estado aceptado
    → POST /internal/orders/{id}/transicion (estado equivalente)
```

La transición de la orden es **agregada**: solo avanza cuando no quedan envíos
pendientes. Así una orden con varias empresas/flotas no se marca `entregada`
hasta que todas lo estén.

---

## 6. API pública

| Método | Ruta | Rol |
|---|---|---|
| PATCH | `/api/v2/orders/{id}/estado` | logística/empresario/admin |
| GET | `/api/v2/admin/envios` | admin (últimas 200 guías) |
| GET | `/api/v2/empresarios/me/envios` | empresario de la empresa activa |
| POST | `/api/v2/empresarios/me/envios/{id}/asignacion` | empresario (modo híbrido) |
| PATCH | `/api/v2/envios/{id}/estado` | flota activa (según `proveedor_tenant_id`) |

La capacidad `fulfillment_delivery` se exige al crear/preparar envíos; el modo
propio exige `logistica_rutas`.

---

## 7. Eventos

### Consume (`logistics.pagos`)

| Evento | Efecto |
|---|---|
| `payment.procesado` | Prepara guía(s) y emite `shipment.started` |
| `order.status.updated` | Si `entregada` → `order.completado`; si `devuelta` → `devolucion.solicitada` |

Idempotencia por `event_id` en `logistics.eventos_procesados`.

### Publica

| Evento | Consumidores |
|---|---|
| `shipment.started` | notifications, intelligence |
| `order.completado` | commissions, analytics, notifications |
| `devolucion.solicitada` | inventory, finance, commissions, notifications |

> `order.completado` se emite desde logística porque es el punto donde la
> entrega física se confirma; es el disparador real de la comisión (RN-04/07).

---

## 8. Seguridad y multi-tenancy

- Toda consulta filtra por `empresa_id`; el avance de envío exige
  `proveedor_tenant_id` igual al tenant activo.
- El empresario solo ve envíos de su empresa; admin ve todos.
- La transición de orden valida que la orden contenga líneas de la empresa
  activa (`ACCESO_DENEGADO` si no).
- Las capacidades se consultan por empresa y negocio.
- `x-internal-key` para llamadas a orders.

---

## 9. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Logs JSON con `order_id`, `guia` y `estado` en las operaciones clave.
- Outbox cada segundo; supervisor RabbitMQ y reintento cada 10 s.
- Apagado ordenado.

### Diagnóstico frecuente

| Síntoma | Causa |
|---|---|
| `ASIGNACION_FIJA` | Se intentó reasignar un envío no híbrido |
| `ENVIO_NO_ENCONTRADO` | Envío de otra empresa o flota |
| Sin guía tras pagar | `payment.procesado` no consumido o capacidad inactiva |
| Orden no avanza | Quedan envíos de otras empresas sin estado final |

---

## 10. Pruebas

```bash
cd packages/logistics-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/envios_pg.rs`: guía idempotente, proveedor por capacidades,
  asignación, avance agregado y transición de orden.
- Tests unitarios de `generar_guia`, validación de estados y montos.

---

## 11. Referencias

- [ADR-0010](../../docs/adr/0010-logistics-service-en-rust.md)
- [ADR-0002](../../docs/adr/0002-envelope-y-desacoplamiento.md)
- [Contrato público](../../docs/openapi/logistics-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/logistics-service/internal.openapi.json)
- [README de paquetes](../README.md)
