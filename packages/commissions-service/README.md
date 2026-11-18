# commissions-service · comisiones y liquidaciones

`commissions-service` calcula y acredita la **comisión de la plataforma** por
línea de venta, reparte el neto entre vendedor y empresario según participación,
compensa devoluciones y ejecuta los **cortes quincenales** (días 1 y 15) con su
libro de liquidaciones.

Es el motor económico del marketplace (RN-04 / RN-06 / RN-07). Se despliega en
**dev y stage**, pero **no en producción**: el portal de empresa se valida en
stage antes de habilitarlo
([ADR-0007](../../docs/adr/0007-commissions-service-en-rust.md)).

- [ADR-0007 · commissions-service en Rust](../../docs/adr/0007-commissions-service-en-rust.md)
- [ADR-0021 · negocios por empresa](../../docs/adr/0021-negocios-por-empresa.md)
- Contrato: [`docs/openapi/commissions-service/`](../../docs/openapi/commissions-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `commissions-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest`, `chrono-tz` |
| Puerto | `3006` (`PORT`) |
| Esquema | `commissions` (lee `identity.negocios`) |
| Outbox | `commissions.outbox` |
| Capacidad | `liquidaciones` |
| Consume | `order.completado`, `payment.procesado`, `devolucion.solicitada`, `comision.acreditada` |
| Publica | `comision.acreditada` |
| Dependencias | `orders` (detalle de orden), `inventory` (KPIs), `identity` (vendedores/capacidades) |
| Workers | corte quincenal cada 30 s (dispara a las 00:05 de los días 1 y 15); outbox 1 s |
| Entornos | dev, **stage** (no producción) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor + outbox |
| `OUTBOX_TABLA` | No | `outbox` (en compose se define `commissions.outbox`) | Outbox |
| `PORT` | No | `3006` | Puerto |
| `SERVICIO` | No | `commissions-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Clientes internos |
| `COMMISSION_RATE` | No | `0.12` | Tasa de plataforma (se interpreta como número JS) |
| `ORDERS_SERVICE_URL` | No | `http://orders-service:3004` | Órdenes |
| `INVENTORY_SERVICE_URL` | No | `http://inventory-service:3013` | KPIs de inventario |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Vendedores/capacidades |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `comisiones.rs` | Cálculo inmutable, acreditación por línea, compensación |
| `liquidaciones.rs` | Periodos quincenales, consolidación, pago y libro |
| `pedidos.rs` | Consumidor de eventos del pedido |
| `ventas.rs` | Estado de cuenta de vendedor y empresa |
| `reportes.rs` | KPIs de ventas/inventario y reporte por negocio |
| `worker.rs` | Disparador del corte quincenal |
| `outbox.rs` / `rabbit.rs` | Eventos y DLQ |
| `http/` | Rutas públicas y guards de rol/capacidad |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `commissions.comisiones` | `order_id`, `linea_clave`, empresa/negocio/tienda/vendedor, venta, comisión, montos vendedor/empresario, participación, estado (`acreditada`/`compensada`) |
| `commissions.pagos` | Pago simulado por orden |
| `commissions.liquidaciones` | Corte por vendedor y periodo |
| `commissions.liquidaciones_empresas` | Corte por empresa/negocio |
| `commissions.compensaciones_devoluciones` | Devoluciones aplicadas a la siguiente liquidación |
| `commissions.eventos_procesados` | Idempotencia |
| `commissions.outbox` | Eventos |

### Invariantes

- `0 < COMMISSION_RATE < 1`; fuera de rango → `TASA_INVALIDA` al calcular.
- Participación entre 0 y 100 (`PARTICIPACION_INVALIDA`).
- Una comisión por `(order_id, linea_clave)`: reintentos no duplican.
- Una comisión `compensada` no vuelve a acreditarse.
- El neto nunca es negativo; el reparto siempre suma exactamente el neto.
- Los montos se persisten en centavos (i32 en tablas históricas).

---

## 4. Cálculo (RN-04 / RN-06)

```
comision      = round(venta × COMMISSION_RATE)          // 12 % por defecto
neto          = venta − comision
vendedor      = round(neto × participacion / 100)
empresario    = neto − vendedor
```

Casos límite:

| Participación | Resultado |
|---|---|
| 100 % | El vendedor recibe todo el neto; empresario 0 |
| 0 % | El vendedor recibe 0; empresario todo el neto |
| 0 < x < 100 | Redondeo como `Math.round`; el residuo va al empresario |

Cada línea de la orden se acredita por separado (`linea_clave`), con su propio
`vendedor_id`, `tienda_id` y participación. Así una orden multi-vendedor genera
varias comisiones.

### Devoluciones

`devolucion.solicitada` registra una **compensación** por la comisión y los
montos de vendedor/empresario. La comisión pasa a `compensada` y el negativo se
aplica en la **siguiente liquidación** (RN-06), nunca se edita el histórico.

---

## 5. Liquidaciones (RN-07)

### Periodos

| Fecha de corte | Periodo cerrado |
|---|---|
| Día 1, 00:05 | 16 → fin del mes anterior |
| Día 15, 00:05 | 1 → 15 del mes actual |
| Día 16+ (manual) | 16 → fin del mes actual |

- Zona horaria local (`TZ`, por defecto `America/Managua`).
- El worker revisa cada **30 s** y ejecuta una sola vez por día.
- `POST /api/v2/admin/liquidaciones/corte` permite forzar un corte manual.
- Al cerrar se consolidan comisiones `acreditada` del periodo, se restan
  compensaciones pendientes y se crean:
  - `commissions.liquidaciones` por vendedor;
  - `commissions.liquidaciones_empresas` por empresa/negocio.
- `POST /api/v2/admin/liquidaciones/{id}/pagar` marca `pagada` (pago simulado
  en MVP).
- Un periodo ya cerrado no se duplica.

---

## 6. API pública

| Método | Ruta | Rol |
|---|---|---|
| GET | `/api/v2/vendedores/me/liquidaciones` | vendedor |
| GET | `/api/v2/vendedores/me/ventas` | vendedor (estado de cuenta) |
| GET | `/api/v2/empresarios/me/liquidaciones` | empresario |
| GET | `/api/v2/empresarios/me/ventas` | empresario |
| GET | `/api/v2/empresarios/me/reportes/negocios` | empresario |
| POST | `/api/v2/admin/liquidaciones/corte` | admin |
| POST | `/api/v2/admin/liquidaciones/{id}/pagar` | admin |
| GET | `/api/v2/admin/reportes` | admin |

La capacidad `liquidaciones` se exige a los roles de empresa/vendedor; admin y
super_admin están exentos del gating.

### Estado de cuenta

Incluye ventas, comisión, monto del vendedor, estado, fecha, comisiones
pendientes y liquidaciones. Los montos salen como strings `"1150.00"`.

### Reportes

- `admin/reportes`: GMV del mes, pedidos, comisiones, vendedores activos,
  productos, stock total, agotados y devoluciones. Si inventory/identity no
  responden, degrada a cero en esas métricas (R-AR-5).
- `empresarios/me/reportes/negocios`: agregado por negocio.

---

## 7. Eventos

### Consume

| Evento | Cola | Efecto |
|---|---|---|
| `payment.procesado` | `commissions.pedidos` | Registra pago simulado |
| `order.completado` | `commissions.pedidos` | Acredita comisión por línea y emite `comision.acreditada` |
| `devolucion.solicitada` | `commissions.pedidos` | Compensa comisión (RN-06) |
| `comision.acreditada` | `commissions.liquidacion` | Consolida para el corte |

Idempotencia por `event_id` en `commissions.eventos_procesados` y por
`(order_id, linea_clave)` en comisiones.

### Publica

`comision.acreditada` con `order_id`, vendedor, montos y empresa. Lo consumen
`finance` (asiento de realización), `intelligence` (rendimiento) y `analytics`.

---

## 8. Seguridad y multi-tenancy

- Vendedor solo ve lo suyo; empresario, lo de su empresa/negocio; admin, todo.
- La capacidad `liquidaciones` se valida contra identity.
- Los eventos de servicio usan un contexto `super_admin` sin tenant, pero las
  consultas fijan el `empresa_id` del payload.
- Los internos de clients llevan `x-internal-key`.
- El pago de liquidación es una transición controlada (`pagada`), no un borrado.

---

## 9. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas HTTP + outbox.
- Logs JSON: `commissions.liquidacion`, `orders`, `outbox`.
- Worker quincenal con log al activarse.
- Apagado ordenado.

### Diagnóstico frecuente

| Síntoma | Causa |
|---|---|
| Comisión no aparece | `order.completado` no consumido o línea sin participación |
| Comisión duplicada | Se ignoró `(order_id, linea_clave)` (no debería ocurrir) |
| Corte no ejecutado | Fecha/hora fuera de 00:05 o ya ejecutado hoy |
| `TASA_INVALIDA` | `COMMISSION_RATE` fuera de `(0,1)` |
| Liquidación no cuadra | Compensación pendiente aplicada al periodo |

---

## 10. Pruebas

```bash
cd packages/commissions-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/comisiones_pg.rs` (~480 líneas): cálculo, acreditación, idempotencia,
  compensación, periodos y pago.
- Tests unitarios de `money`, `calcular_con_tasa`, `periodo_de`,
  `periodo_a_cerrar` y el disparador del worker.

---

## 11. Referencias

- [ADR-0007](../../docs/adr/0007-commissions-service-en-rust.md)
- [Contrato público](../../docs/openapi/commissions-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/commissions-service/internal.openapi.json)
- [README de paquetes](../README.md)
