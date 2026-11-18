# finance-service · finanzas, contabilidad y comercio

`finance-service` es el **bounded context financiero**. Cubre cinco áreas:

1. **Contabilidad** por partida doble, con bitácora append-only.
2. **Tributación** multijurisdicción (Nicaragua: Ley 822) y declaraciones.
3. **Facturación** fiscal DGI (series, secuenciales, IVA).
4. **Comercio**: bancos, créditos, gastos, cuentas por pagar, proveedores,
   compras, órdenes de compra, caja (movimientos, cierres, caja chica,
   transferencias), clientes y etiquetas de código de barras.
5. **Planeación**: proyecciones, punto de equilibrio, KPIs, tablero de alertas
   y estudios de sensibilidad.

- [ADR-0014 · finance-service en Rust](../../docs/adr/0014-finance-service-en-rust.md)
- Contrato: [`docs/openapi/finance-service/`](../../docs/openapi/finance-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `finance-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest`, `urlencoding` |
| Puerto | `3007` (`PORT`) |
| Esquema | `finance` (lee `identity.negocios`/`partes`) |
| Outbox | `finance.outbox` |
| Capacidades | `facturacion`, `ventas_credito` |
| Consume | `order.created`, `payment.procesado`, `comision.acreditada`, `devolucion.solicitada` (cola `finance.pedidos`) |
| Publica | `asiento.registrado`, `declaracion.generada`, `comprobante.emitido` |
| Dependencias | `orders` (lectura de órdenes), `identity` (vendedores/capacidades) |
| Worker | declaraciones del mes anterior el día 1 a las 00:10 |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor + outbox |
| `OUTBOX_TABLA` | No | `outbox` (en compose se define `finance.outbox`) | Outbox |
| `PORT` | No | `3007` | Puerto |
| `SERVICIO` | No | `finance-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Internos + clientes |
| `ORDERS_SERVICE_URL` | No | `http://orders-service:3004` | Órdenes |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Vendedores/capacidades |
| `COMMISSION_RATE` | No | `0.12` | Comisión (RN-04) |
| `EMITIR_COMPROBANTES_FISCALES` | No | `false` | Emite `EMITIDO` en vez de `BORRADOR` |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `contabilidad.rs` | Plan de cuentas, asientos, partida doble, anulación y libros |
| `tributacion.rs` | Jurisdicciones, regímenes, sujetos, declaraciones IVA/IR/CUOTA_FIJA |
| `facturacion.rs` | Series, secuenciales, comprobantes, IVA 15 %, notas de crédito |
| `comercio.rs` + `comercio/` | 16 recursos del libro de operación |
| `comercio/creditos.rs` | Cuentas de crédito por parte y venta a crédito desde orders |
| `comercio/caja.rs` | Movimientos, cierre diario, caja chica, transferencias |
| `comercio/gastos.rs` | Gastos, proveedores, compras y órdenes de compra |
| `finanzas/kpis.rs` | KPIs del tablero (GMV, comisiones, devoluciones, churn…) |
| `finanzas/proyecciones.rs` | Modelo mes a mes, VAN/TIR/payback, sensibilidad |
| `finanzas/alertas.rs` | Capas, umbrales y acciones del tablero |
| `consumidor.rs` | Eventos → asientos, métricas y comprobantes |
| `http/` | Finanzas, comercio e internos |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `finance.plan_cuentas` | Plan contable (código jerárquico, nombre, tipo, estado) |
| `finance.asientos`, `asiento_detalles` | Cabecera y líneas debe/haber |
| `finance.asientos_vista`, `libro_mayor_mv` | Proyecciones de lectura |
| `finance.comprobantes`, `series_comprobantes` | Facturación fiscal |
| `finance.jurisdicciones`, `regimenes_fiscales`, `sujetos_fiscales` | Configuración tributaria |
| `finance.declaraciones` | IVA/IR/cuota por periodo |
| `finance.credito_cuentas` | Límite y estado de crédito por parte |
| `finance.workspace_registros` | 16 recursos de comercio en JSONB con ámbito tenant/vendedor |
| `finance.proyecciones` | Supuestos y resultados |
| `finance.metricas_mensuales`, `metricas_vendedores` | KPIs materializados |
| `finance.tipos_cambio` | Tipo de cambio por moneda/fecha |
| `finance.eventos_procesados` | Idempotencia |
| `finance.outbox` | Eventos |

---

## 4. Contabilidad

### Partida doble

`POST /api/v2/finanzas/asientos` valida:

- Al menos **dos** movimientos (`PARTIDA_DOBLE_INVALIDA`).
- `debe > 0` y `debe == haber` (`PARTIDA_DOBLE_DESCUADRADA`).
- Tipo de asiento permitido (`ingreso`, `gasto`, `ajuste`, `apertura`,
  `cierre`, etc.; un tipo inválido → `TIPO_ASIENTO_INVALIDO`).
- Cuentas existentes y activas.

Un asiento **nunca se edita ni se borra**: `POST /asientos/{id}/anular` lo marca
como `ANULADO` (la fila y sus detalles permanecen para auditoría) y deja de
contar en los libros. Cada asiento emite `asiento.registrado`.

### Libros

- `GET /finanzas/libro-mayor/{cuenta}`: movimientos por cuenta.
- `GET /finanzas/libro-ventas` y `/libro-ventas/negocios` (DGI).
- `GET /finanzas/libro-compras` (DGI).
- `GET /finanzas/asientos`: libro diario con filtros.

### Asientos automáticos del consumidor

| Evento | Asiento |
|---|---|
| `payment.procesado` | Debe `1.1.1 Caja` / Haber `2.1.2 Fondos por liquidar`; suma GMV |
| `comision.acreditada` | `2.1.2 Fondos por liquidar` → `4.1 Ingresos por comisiones`, `2.1.1 Comisiones por pagar`, `2.1.6` (parte empresa) |
| `devolucion.solicitada` | Ajuste reverso de la comisión (RN-06) |
| `order.created` | Métrica de embudo (checkout completado), sin asiento |

Todas las escrituras van en una transacción con outbox e idempotencia por
`event_id`.

---

## 5. Tributación

- **Jurisdicciones** configurables (`codigo_pais`, moneda, `tasa_iva`,
  `tasa_ir`, periodicidad, leyes, activa). Nicaragua sembrada con IVA 15 % e IR
  mensual (Ley 822).
- **Configuración fiscal por tenant** (`finance.configuracion_fiscal`):
  `GET/PUT /finanzas/configuracion-fiscal` fija la jurisdicción por empresa y,
  opcionalmente, por negocio. La resolución es negocio → empresa → primera
  activa (compatibilidad legacy); ya no depende del orden alfabético.
- **Comprobantes**: la moneda y el IVA salen de la jurisdicción de la serie; el
  IVA se calcula **por línea** con el precio, cantidad y descuento de la orden
  (no sobre un total ciego). La serie por negocio sigue mandando.
- **Regímenes fiscales** por jurisdicción (RAMPRA, cuota fija, etc.).
- **Sujetos tributarios** por usuario (RUC, razón social, régimen, estado).
- **Declaraciones**: `IVA`, `IR` y `CUOTA_FIJA` informativa.
  - `POST /finanzas/declaraciones/generar` calcula ingresos/gastos del periodo.
  - Idempotente por `(jurisdiccion, tipo, periodo_inicio)`.
  - Estados: `GENERADA`, `PRESENTADA`, `PAGADA`.
  - `POST /declaraciones/{id}/presentar` y `/pagar`.
  - **Límite actual:** el libro mayor no tiene `empresa_id`; las declaraciones
    siguen siendo de plataforma. La configuración por tenant ya aplica a
    comprobantes y a `jurisdiccion_de`.
- **Planificador**: el día **1** a partir de las **00:10** genera las
  declaraciones del mes anterior (una sola vez por periodo, en hora local).

> La moneda de facturación sale de la jurisdicción configurada (por defecto
> `NIO` para Nicaragua); el servicio no convierte automáticamente salvo que se
> registre un tipo de cambio.

---

## 6. Facturación

- **Series** por negocio/tipo con `prefijo`, `secuencial_actual` y `ruc_emisor`.
- `POST /finanzas/comprobantes` emite factura o nota de crédito:
  - `numero = prefijo + secuencial(8)`;
  - el secuencial se consume **siempre**, incluso en `BORRADOR`, para no romper
    la numeración;
  - `IVA = base_gravada × tasa` (15 % por defecto);
  - total = base gravada + IVA + exento.
- Estado `EMITIDO` si `EMITIR_COMPROBANTES_FISCALES=true`; si no, `BORRADOR`.
- Emitir un comprobante genera el asiento contable y `comprobante.emitido`.
- `POST /comprobantes/{id}/anular` conserva el registro y ajusta la
  contabilidad.
- `GET /comprobantes/series` y `GET /comprobantes` con filtros.

---

## 7. Comercio (libro de operación)

Los 16 recursos de `finance.workspace_registros`:

`bancos-cuentas`, `creditos`, `gastos`, `cuentas-por-pagar`, `proveedores`,
`compras`, `ordenes-compra`, `caja-metodos-pago`, `caja-movimientos`,
`caja-cierres`, `caja-chica-fondo`, `caja-chica-movimientos`,
`caja-transferencias`, `configuracion-documentos`, `clientes`, `codigos-barras`.

Cada registro tiene **ámbito**: de empresa o de vendedor. El servicio valida
que el actor pueda operar ese ámbito.

### Créditos

- `finance.credito_cuentas` define `limite_cents` y `activo` por parte.
- `PUT /comercio/credito-cuentas/{parteId}` fija el límite.
- `POST /internal/finance/creditos` registra una venta a crédito desde orders:
  valida cupo, crea el registro y descuenta el disponible.
- `POST /comercio/creditos/{id}/abonar` acepta abonos parciales; rechaza
  `ABONO_EXCEDE_SALDO` y `ABONO_INVALIDO`.
- Anular una orden no creada borra el crédito ligado
  (`DELETE /internal/finance/creditos/orden/{orderId}`).

### Caja

- Métodos de pago propios de caja (no confundir con `orders.metodos_pago`).
- Movimientos de ingreso/egreso; anulación y borrado solo sin movimientos.
- `GET /comercio/caja/cierres/preview?date=` calcula apertura, ingresos,
  egresos, saldo esperado y diferencia contra el conteo.
- `POST /comercio/caja/cierres` persiste el cierre; **un solo cierre por
  fecha** (`CIERRE_EXISTENTE`).
- Caja chica: fondo + movimientos.
- Transferencias internas caja ↔ bancos, anulables.
- Configuración de documentos imprimibles (logo, cabecera, pie, RUC,
  dirección, teléfono).

### Compras

- Compra por líneas, compra directa (gasto no inventariable) y compra de
  producto nuevo.
- Orden de compra → `convertir` genera la compra; `anular` la cancela.
- Numeración `C-…` para compras y `OC-…` para órdenes.
- **No mueve stock por sí mismo**: el inventario se registra en
  `inventory-service`; aquí queda el registro comercial/financiero.

---

## 8. Proyecciones, KPIs y tablero

### Proyecciones

`POST /finanzas/proyecciones` acepta supuestos:

```json
{
  "horizonte_meses": 24,
  "vendedores_iniciales": 20,
  "entrada_vendedores_mes": 5,
  "churn_tasa": 0.03,
  "pedidos_por_vendedor": [25, 27, 30],
  "ticket_promedio": "850.00",
  "comision_tasa": 0.12,
  "costos_fijos": "12000.00",
  "inversion_inicial": "50000.00",
  "tasa_descuento_mensual": 0.02
}
```

Calcula mes a mes: GMV, comisión, costos, margen, caja; indicadores **VAN**,
**TIR**, **payback** y margen de seguridad.

Endpoints: `GET/POST /proyecciones`, `GET /proyecciones/{id}`,
`GET /proyecciones/sensibilidad`, `GET /proyecciones/plan-bienal`,
`GET /punto-equilibrio`.

### KPIs (`GET /finanzas/kpis`)

GMV, pedidos pagados/entregados, ingresos por comisiones, devoluciones,
vendedores activos, pedidos por vendedor, activación a 15 días, tasa de
devoluciones, churn mensual, conversión carrito/checkout y liquidación a
tiempo.

### Tablero (`GET /finanzas/tablero`)

Combina KPIs + alertas por capa (adquisición, activación, retención,
monetización, operación), cada una con frecuencia, umbral, valor, acción y
fecha esperada. Genera `TableroAlerta` con estado `ok`/`alerta`/`sin_dato`.

---

## 9. API pública (resumen)

> Fuente de verdad: `docs/openapi/finance-service/public.openapi.json`.

| Área | Rutas |
|---|---|
| Contabilidad | `GET/POST /finanzas/cuentas`, `POST /cuentas/{codigo}/estado`, `GET/POST /asientos`, `POST /asientos/{id}/anular`, `GET /libro-mayor/{cuenta}`, `GET /libro-ventas[/negocios]`, `GET /libro-compras` |
| Fiscal | `GET/POST /comprobantes`, `GET /comprobantes/series`, `POST /comprobantes/{id}/anular`, `GET/POST /jurisdicciones`, `GET/PUT /configuracion-fiscal`, `GET/POST /regimenes`, `GET/POST /sujetos`, `POST /sujetos/{id}/baja`, `GET /declaraciones`, `POST /declaraciones/generar`, `POST /declaraciones/{id}/presentar`, `POST /declaraciones/{id}/pagar`, `GET /vendedores/me/fiscal`, `GET /tipos-cambio`, `POST /admin/tipos-cambio` |
| Planeación | `GET/POST /proyecciones`, `GET /proyecciones/{id}`, `/sensibilidad`, `/plan-bienal`, `GET /punto-equilibrio`, `/kpis`, `/tablero` |
| Bancos/créditos | `GET/POST /comercio/bancos-cuentas`, `PATCH/DELETE .../{id}`, `GET/POST /comercio/creditos`, `POST .../{id}/pagar`, `/abonar`, `DELETE .../{id}`, `GET /comercio/credito-cuentas`, `PUT .../{parteId}` |
| Gastos/compras | `GET/POST /comercio/gastos`, `PATCH/DELETE .../{id}`, `POST .../{id}/{pagar,cancelar}`, `GET/POST /comercio/cuentas-por-pagar`, `POST .../{id}/pagar`, `GET/POST /comercio/proveedores`, `GET/POST /comercio/compras`, `POST /compras/{directas,producto-nuevo}`, `POST /compras/{id}/anular`, `GET/POST /comercio/ordenes-compra`, `POST .../{id}/{convertir,anular}` |
| Caja | `GET/POST/PATCH/DELETE /comercio/caja/metodos-pago`, `GET/POST /caja/movimientos`, `POST /caja/movimientos/{id}/anular`, `GET/POST /caja/cierres`, `GET /caja/cierres/preview`, `GET/PUT /caja/chica/fondo`, `GET/POST /caja/chica/movimientos`, `GET/POST /caja/transferencias`, `POST /caja/transferencias/{id}/anular` |
| Clientes/etiquetas | `GET/POST /comercio/clientes`, `POST /clientes/importar`, `PATCH/DELETE /clientes/{id}`, `GET/POST /comercio/codigos-barras`, `DELETE .../{id}`, `GET/PUT /comercio/configuracion-documentos` |

---

## 10. API interna (`x-internal-key`)

| Método | Ruta | Consumidor |
|---|---|---|
| GET | `/internal/finance/sujetos/{usuarioId}` | identity (situación fiscal) |
| GET | `/internal/finance/tipos-cambio` | orders/servicios |
| POST | `/internal/finance/creditos` | orders (venta a crédito) |
| DELETE | `/internal/finance/creditos/orden/{orderId}` | orders (compensación) |
| GET | `/internal/finance/declaraciones` | reportes (orders/commissions) |

---

## 11. Eventos

### Consume (`finance.pedidos`)

| Evento | Efecto |
|---|---|
| `order.created` | Métrica de embudo |
| `payment.procesado` | Asiento Caja/Fondos + métrica GMV |
| `comision.acreditada` | Asiento de realización de ingreso |
| `devolucion.solicitada` | Asiento de ajuste/reverso |

Idempotencia por `event_id` en `finance.eventos_procesados`.

### Publica

| Evento | Cuándo |
|---|---|
| `asiento.registrado` | Cada asiento (manual o automático) |
| `declaracion.generada` | Generación de IVA/IR/cuota |
| `comprobante.emitido` | Emisión fiscal |

---

## 12. Seguridad y multi-tenancy

- Filtro por `empresa_id` y `negocio_id`; los recursos de comercio respetan el
  ámbito vendedor/empresa.
- Roles de comercio/finanzas + permisos; admin global para catálogos fiscales.
- Las cuentas de crédito son por parte y empresa.
- Los comprobantes y series respetan el negocio emisor.
- `x-internal-key` en las rutas internas.
- El dinero se maneja en centavos; se rechazan montos negativos y desbordes.

---

## 13. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas HTTP + outbox + gauges de tablero.
- Logs JSON con IDs de orden/asiento.
- Worker de declaraciones (cada 60 s, dispara una vez al mes).
- Outbox cada segundo; supervisor RabbitMQ y reintento cada 10 s.
- Apagado ordenado.

---

## 14. Pruebas

```bash
cd packages/finance-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/finance_pg.rs` (~865 líneas): contabilidad, tributación,
  facturación, créditos, caja, compras y proyecciones contra PostgreSQL real.
- `comercio/tests.rs`: normalización de compras, ámbitos y totales.
- Casos puros de proyecciones (VAN/TIR/payback) y alertas.

---

## 15. Referencias

- [ADR-0014](../../docs/adr/0014-finance-service-en-rust.md)
- [Contrato público](../../docs/openapi/finance-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/finance-service/internal.openapi.json)
- [README de paquetes](../README.md)

### Facturación electrónica y PDF (opt-in)

- La capacidad `facturacion_electronica` habilita la función por negocio.
- `PUT /finanzas/configuracion-fiscal` acepta `facturacion_electronica`,
  `formato_pdf`, `proveedor_electronico` (`simulado|http`) y
  `ambiente_electronico` (`pruebas|produccion`).
- Con el flag activo, `emitir` llama al proveedor y guarda
  `electronica_estado`/`electronica_referencia`; un fallo deja el comprobante
  con estado `error` sin perder numeración.
- `GET /finanzas/comprobantes/{id}/pdf` descarga el PDF si `formato_pdf=true`;
  sin el flag responde `PDF_NO_ACTIVADO`.
- El proveedor HTTP se configura con `FACTURACION_PROVEEDOR_URL` y
  `FACTURACION_PROVEEDOR_KEY`; DGI concreta se conecta detrás de esa URL.
