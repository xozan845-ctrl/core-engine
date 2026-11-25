# commissions-service · comisiones y liquidaciones

`commissions-service` calcula y acredita la **comisión de la plataforma** por
línea de venta, reparte el neto entre vendedor y empresario según participación,
compensa devoluciones y ejecuta el **cierre diario** con su libro de
liquidaciones y payouts al negocio y al vendedor.

Es el motor económico del marketplace (RN-04 / RN-06 / RN-07). Se despliega en
**dev, stage y producción** desde ADR-0053: la plataforma cobra centralizado y
dispersa diariamente.

- [ADR-0007 · commissions-service en Rust](../../docs/adr/0007-commissions-service-en-rust.md)
- [ADR-0021 · negocios por empresa](../../docs/adr/0021-negocios-por-empresa.md)
- [ADR-0053 · cobro centralizado y dispersión diaria](../../docs/adr/0053-cobro-centralizado-dispersion-diaria.md)
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
| Consume | `order.completado`, `payment.procesado`, `devolucion.solicitada`, `devolucion.recibida`, `comision.acreditada` |
| Publica | `comision.acreditada`, `venta.directa.acreditada` |
| Dependencias | `orders` (detalle de orden), `inventory` (KPIs), `identity` (vendedores/capacidades) |
| Workers | cierre diario a la hora configurada (00:10 por defecto); outbox 1 s |
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
| `TILOPAY_DISPERSION_URL` | Para pagar con Tilopay | — | Base de la API de dispersión |
| `TILOPAY_DISPERSION_PATH` | No | `/payouts` | Ruta de creación de payout |
| `TILOPAY_DISPERSION_TOKEN` | Para pagar con Tilopay | `TILOPAY_KEY` | Bearer de dispersión |
| `PAGADITO_DISPERSION_URL` | Para pagar con Pagadito | — | Base de la API de dispersión |
| `PAGADITO_DISPERSION_PATH` | No | `/payouts` | Ruta de creación de payout |
| `PAGADITO_DISPERSION_TOKEN` | Para pagar con Pagadito | — | Bearer de dispersión |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `comisiones.rs` | Cálculo inmutable, acreditación por línea, compensación |
| `liquidaciones.rs` | Periodos, consolidación, pago y libro |
| `dispersion.rs` | Cierre diario, payouts, cuentas y adaptadores Tilopay/Pagadito |
| `pedidos.rs` | Consumidor de eventos del pedido |
| `ventas.rs` | Estado de cuenta de vendedor y empresa |
| `reportes.rs` | KPIs de ventas/inventario y reporte por negocio |
| `worker.rs` | Disparador del cierre y envío diario |
| `outbox.rs` / `rabbit.rs` | Eventos y DLQ |
| `http/` | Rutas públicas y guards de rol/capacidad |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `commissions.comisiones` | `order_id`, `linea_clave`, empresa/negocio/tienda/vendedor, venta, comisión, montos vendedor/empresario, participación, estado (`acreditada`/`compensada`) |
| `commissions.pagos` | Pago por orden con fee de pasarela, impuestos y neto liquidado |
| `commissions.liquidaciones` | Corte por vendedor y periodo |
| `commissions.liquidaciones_empresas` | Corte por empresa/negocio |
| `commissions.cuentas_pago` | Cuenta/token de destino por beneficiario y proveedor |
| `commissions.payouts` | Payout diario por beneficiario y su estado |
| `commissions.payout_items` | Liquidaciones incluidas en cada payout |
| `commissions.configuracion_dispersion` | Proveedor, hora, mínimo y retención |
| `commissions.compensaciones_devoluciones` | Devoluciones aplicadas a la siguiente liquidación |
| `commissions.eventos_procesados` | Idempotencia |
| `commissions.outbox` | Eventos |

### Invariantes

- `0 <= COMMISSION_RATE < 1`; fuera de rango → `TASA_INVALIDA` al calcular.
  La tasa `0` es válida y desactiva la comisión de plataforma (venta directa).
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

`devolucion.solicitada` registra una **compensación** total por la comisión y
los montos de vendedor/empresario. `devolucion.recibida` compensa solo las
cantidades devueltas por una RMA. La compensación es acumulativa y nunca supera
la cantidad ni los montos originales; el negativo se aplica en la **siguiente
liquidación** (RN-06), nunca se edita el histórico.

---

## 5. Liquidaciones y dispersión diaria

### Cierre diario

- El worker revisa cada **30 s** y, a la hora local configurada (`00:10` por
  defecto), cierra el día anterior.
- `POST /api/v2/admin/dispersion/cerrar-dia?fecha=YYYY-MM-DD` permite forzar el
  cierre de una fecha.
- Al cerrar se consolidan comisiones `acreditada` del día, se restan
  compensaciones pendientes y se crean:
  - `commissions.liquidaciones` por vendedor;
  - `commissions.liquidaciones_empresas` por empresa/negocio.
- Luego se crea un **payout por beneficiario**:
  - vendedor: su comisión neta;
  - negocio/empresa: el remanente tras la comisión de plataforma.
- La retención configurada (`retencion_bps`) se descuenta del payout y el neto
  queda en `monto_neto_cents`.
- `POST /api/v2/admin/dispersion/payouts/{id}/enviar` envía al proveedor
  configurado. `POST .../{id}/confirmar` lo marca confirmado y sus
  liquidaciones pasan a `pagada`.
- El fee de pasarela es costo de la plataforma: no se descuenta del payout del
  negocio ni del vendedor.

### Periodos históricos

`POST /api/v2/admin/liquidaciones/corte` sigue disponible para cortes
quincenales/manuales. La dispersión diaria usa periodos de un día.

---

## 6. API pública

| Método | Ruta | Rol |
|---|---|---|
| GET | `/api/v2/vendedores/me/liquidaciones` | vendedor |
| GET | `/api/v2/vendedores/me/ventas` | vendedor (estado de cuenta) |
| GET | `/api/v2/empresarios/me/liquidaciones` | empresario |
| GET | `/api/v2/empresarios/me/ventas` | empresario |
| GET | `/api/v2/empresarios/me/reportes/negocios` | empresario |
| GET | `/api/v2/empresarios/me/dispersion` | empresario |
| GET | `/api/v2/vendedores/me/dispersion` | vendedor |
| GET/PUT | `/api/v2/admin/dispersion/configuracion` | admin |
| GET/POST | `/api/v2/admin/dispersion/cuentas` | admin |
| GET | `/api/v2/admin/dispersion/payouts` | admin |
| POST | `/api/v2/admin/dispersion/cerrar-dia` | admin |
| POST | `/api/v2/admin/dispersion/payouts/{id}/enviar` | admin |
| POST | `/api/v2/admin/dispersion/payouts/{id}/confirmar` | admin |
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
| `order.completado` | `commissions.pedidos` | Acredita comisión o emite `venta.directa.acreditada` por línea |
| `devolucion.solicitada` | `commissions.pedidos` | Compensa lo pendiente por devolución total (RN-06) |
| `devolucion.recibida` | `commissions.pedidos` | Compensa proporcionalmente las cantidades RMA |
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
| `TASA_INVALIDA` | `COMMISSION_RATE` fuera de `[0,1)` |
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

### Devoluciones parciales

- `devolucion.recibida` compensa proporcionalmente la comisión de cada línea
  (`linea_clave` + `cantidad`), usando `commissions.comisiones.cantidad`.
- Si la cantidad devuelta cubre la línea completa, la comisión pasa a
  `compensada`; si es parcial, sigue `acreditada` y acumula compensaciones.
