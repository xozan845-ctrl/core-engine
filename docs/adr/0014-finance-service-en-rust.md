# ADR-0014: Reescritura de `finance-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación en curso
- **Ámbito**: `finance-service` (HTTP público e interno, PostgreSQL/RLS,
  RabbitMQ, Outbox, contabilidad, tributación, facturación, comercio y métricas)
- **Referencias**: [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0004](./0004-scheduling-service-en-rust.md),
  [ADR-0006](./0006-notifications-service-en-rust.md),
  [ADR-0008](./0008-field-service-en-rust.md),
  [ADR-0010](./0010-logistics-service-en-rust.md)

## Contexto

`finance-service` es el servicio NestJS de finanzas (~4.000 líneas de código de
dominio y ~7.200 contando controladores y DTO) que:

- Expone la operación de comercio del workspace (`/api/v2/comercio/**`):
  bancos, créditos, gastos, cuentas por pagar, proveedores, compras, órdenes de
  compra, caja (métodos, movimientos, cierres, caja chica y transferencias),
  configuración de documentos, clientes y códigos de barras.
- Lleva contabilidad por partida doble (`/api/v2/finanzas/cuentas`,
  `asientos`, `libro-mayor`, `libro-ventas`, `libro-compras`), con bitácora
  append-only y publicación de `asiento.registrado` vía Outbox.
- Calcula el modelo financiero del cap. 8: proyecciones, punto de equilibrio,
  KPIs, sensibilidad, plan bienal, tablero de alertas y situación fiscal del
  vendedor.
- Administra el régimen fiscal multijurisdicción: jurisdicciones, regímenes,
  sujetos tributarios y declaraciones mensuales (Ley 822: IVA 15 % e IR).
- Emite comprobantes fiscales DGI con series y secuenciales consecutivos
  (`BORRADOR`/`EMITIDO`/`ANULADO`).
- Consume la cola `finance.pedidos` (`order.created`, `payment.procesado`,
  `comision.acreditada`, `devolucion.solicitada`) para alimentar la
  contabilidad y las métricas CQRS por mes y por vendedor, con idempotencia
  por `event_id`.
- Mantiene la superficie interna `internal/finance/**` (situación fiscal,
  registro y anulación de créditos de una orden, declaraciones) protegida por
  `x-internal-key`.
- Emite `envelope {data, meta}` (R-C-1), errores `{codigo, mensaje, detalles?}`,
  métricas Prometheus y `/health`.

El servicio **no tiene migración en curso**: no existe rama, worktree, ADR ni
crate previo para `finance-service` (verificado con `git worktree list`,
`git branch -a` y `docs/adr/`). Las migraciones abiertas son `stores` (ADR-0011)
y `orders`; este ADR no invade su alcance.

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismas rutas, métodos y códigos; envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores `{codigo, mensaje, detalles?}` sin `meta` |
| Autorización | roles `super_admin`, `admin`, `empresario`, `vendedor`; capacidades `facturacion` y `ventas_credito`; tenant activo obligatorio en el ámbito de empresa |
| Ámbito comercio | empresa (`tenant_id`) o vendedor (`propietario_id`); los recursos `gastos`, `proveedores`, `compras`, `ordenes-compra`, `caja-*` y `configuracion-documentos` son solo de empresa |
| Persistencia | `finance.workspace_registros`, `credito_cuentas`, `asientos`, `asiento_detalles`, `asientos_vista`, `libro_mayor_mv`, `plan_cuentas`, `jurisdicciones`, `regimenes_fiscales`, `sujetos_fiscales`, `declaraciones`, `comprobantes`, `series_comprobantes`, `proyecciones`, `metricas_mensuales`, `metricas_vendedores`, `eventos_procesados` y `outbox` |
| Montos | enteros en centavos; `Money` para formateo `"1150.00"`; nunca punto flotante salvo el redondeo explícito de tasas, idéntico a `Math.round` |
| Contabilidad | partida doble (`debe == haber`, al menos dos líneas); bitácora append-only; anulación solo de estado; asientos `INGRESO/EGRESO/AJUSTE/CIERRE/APERTURA/MANUAL` |
| Tributación | tasas de la jurisdicción activa (`tasa_iva`, `tasa_ir`); declaraciones idempotentes por `(jurisdiccion, tipo, periodo_inicio)`; sin actividad no se declara |
| Facturación | serie activa por tipo; secuencial consecutivo consumido incluso en `BORRADOR`; `EMITIR_COMPROBANTES_FISCALES` decide `EMITIDO` |
| Crédito de orden | idempotente por `order_id`; rechazo 409 si no hay cuenta activa o cupo; compensación por `order_id` |
| Eventos | mismos nombres y payloads; idempotencia por `event_id`; asiento, métricas y marcado comparten transacción; Outbox con lote de 50 y `pendiente→publicado` |
| RabbitMQ | exchanges `core-engine.events`/`core-engine.dlq`; cola `finance.pedidos` con sus cuatro routing keys; reintentos `x-intentos` máx. 3 y DLQ con republicación cada 10 s |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus con los mismos nombres; logs JSON con `request_id`, `servicio` y nivel |
| Fechas | ISO 8601 UTC con milisegundos y `Z` (R-C-4); fechas de negocio `YYYY-MM-DD` |
| Runtime | puerto 3007 (`PORT`), mismas variables de entorno, fail-fast si falta `DATABASE_URL` o `RABBITMQ_URL`, cierre ordenado |

### 2. Implementación destino

- Crate en la raíz de `packages/finance-service/`, con binario
  `finance-service`; se retira el TypeScript en la conmutación.
- `axum` + `tokio` para HTTP, `sqlx` para PostgreSQL, `lapin` para RabbitMQ,
  `reqwest` para las APIs internas, `serde`/`serde_json`, `uuid`, `chrono`,
  `thiserror` y `urlencoding`.
- Capas: `http/` (router, envelope, rutas), `comercio.rs`, `contabilidad.rs`,
  `tributacion.rs`, `facturacion.rs`, `finanzas/` (proyecciones, KPIs,
  alertas), `consumidor.rs`, `metricas.rs`, `outbox.rs`, `rabbit.rs`,
  `clientes.rs`, `auth.rs`, `db.rs`, `config.rs`.
- Inyección explícita por estado compartido (`AppState` con `Arc`), equivalente
  verificable de la DI de NestJS (enmienda de R-MS-3).

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y
   `cargo test` verdes.
2. Pruebas de integración PostgreSQL (`tests/finance_pg.rs`): partida doble y
   libro mayor, comercio genérico con aislamiento por tenant/vendedor,
   crédito de orden idempotente, declaraciones idempotentes, comprobantes con
   secuencial consecutivo y consumidor de eventos idempotente.
3. Los OpenAPI versionados no cambian; el binario sirve el mismo documento.
4. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR y el índice | Decisión registrada |
| 2 | Andamiaje Rust: configuración, HTTP, métricas, envelope, errores y contabilidad | `cargo test` verde |
| 3 | Modelo financiero: proyecciones, KPIs, alertas, tablero y vendedor fiscal | Cálculos y tablero verificados |
| 4 | Comercio, tributación y facturación con su superficie interna | Integración PostgreSQL verde |
| 5 | Consumidor, métricas, Outbox, imagen Docker, compose y cobertura | Build y gates verdes |
| 6 | Conmutación: retirar TypeScript y actualizar scripts raíz | `npm run demo` y OpenAPI verdes |
| 7 | Smoke de `staging` | Imagen Rust validada antes de producción |

## Consecuencias

### Positivas

- Un servicio NestJS menos y menos consumo en reposo.
- Tipado estricto en montos, tasas, asientos y declaraciones.
- Reutiliza el patrón y los gates de ADR-0004/ADR-0006/ADR-0010.

### Costes y riesgos

- `comercio` es un libro genérico JSONB: la paridad depende de conservar los
  defaults y los totales calculados en cada recurso.
- La contabilidad y la tributación son auditables (DGI): cualquier diferencia
  de redondeo debe quedar cubierta por pruebas.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  integración PostgreSQL en verde.
- Partida doble, declaraciones y comprobantes conservan sus invariantes.
- OpenAPI, salud y métricas conservan el contrato.
- Smoke de `staging` verde en la primera promoción.
