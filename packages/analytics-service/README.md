# analytics-service · analítica de negocio, empresa y plataforma

`analytics-service` es el **único dueño de la analítica agregada**: hechos de
venta, comisiones, cohortes de retención, evolución mensual, comparación entre
negocios, vista de plataforma y reportes programados.

No renderiza nada: entrega JSON o CSV con totales y desgloses. La empresa sigue
siendo el límite de datos; el negocio es la unidad de comparación.

- [ADR-0022 · analytics-service](../../docs/adr/0022-analytics-service.md)
- [ADR-0021 · negocios por empresa](../../docs/adr/0021-negocios-por-empresa.md)
- Contrato: [`docs/openapi/analytics-service/`](../../docs/openapi/analytics-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `analytics-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest`, `chrono-tz` |
| Puerto | `3016` (`PORT`) |
| Esquema | `analytics` (lee `identity.empresas`/`negocios`) |
| Outbox | No publica; `analytics.eventos_procesados` para idempotencia |
| Capacidad | `analitica_negocios` (activa por defecto en verticales/packs/empresas) |
| Consume | `order.completado` (cola `analytics.orders`), `comision.acreditada` (cola `analytics.comisiones`) |
| Dependencias | `orders` (detalle de la orden), `identity` (capacidades) |
| Worker | reportes vencidos cada 5 minutos |
| Entornos | dev, stage, producción |
| Expuesto al host | No (el gateway aún no tiene destino `/api/v2/analytics/**`) |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor |
| `PORT` | No | `3016` | Puerto |
| `SERVICIO` | No | `analytics-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Internos + orders |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Capacidades |
| `ORDERS_SERVICE_URL` | No | `http://orders-service:3004` | Hechos de orden |
| `OUTBOX_TABLA` | No | `outbox` | No usado para publicar |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

> **Importante:** el API Gateway actual **no enruta** `/api/v2/analytics/**`.
> El servicio funciona y consume eventos, pero esa superficie no es alcanzable
> desde el borde hasta que se añada un `Destino` en `proxy.rs`. Es una deuda
> documentada.

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `analitica.rs` | Resumen por negocio, empresa, plataforma, evolución y cohortes |
| `consumidor.rs` | `order.completado` / `comision.acreditada` → hechos |
| `reportes.rs` | Programación, ejecución y persistencia de reportes |
| `clients.rs` | Cliente interno de orders |
| `http/analitica.rs` | Endpoints públicos (JSON/CSV) |
| `http/interno.rs` | Ejecución de reportes vencidos |
| `auth.rs` | Roles + capacidad `analitica_negocios` |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `analytics.hechos_venta` | Un hecho por línea/orden: empresa, negocio, comprador, vendedor, tienda, SKU, montos, fecha |
| `analytics.comisiones` | Comisión acreditada (empresa, negocio, vendedor, monto) |
| `analytics.reportes_programados` | Nombre, tipo, formato, frecuencia, destino, próxima ejecución, activo |
| `analytics.reporte_ejecuciones` | Resultado, estado y error de cada ejecución |
| `analytics.eventos_procesados` | Idempotencia |
| `analytics.outbox` | Creado por la migración; no se publica |

### Invariantes

- Cada hecho pertenece a una empresa y, si aplica, a un negocio.
- `event_id` es único en `eventos_procesados`: un evento no duplica hechos.
- Los montos se guardan en centavos.
- La analítica de plataforma solo la ven `super_admin`/`admin`.
- Un reporte siempre pertenece a una empresa y tiene un `destino` textual de
  3–200 caracteres (el envío real queda fuera del servicio).

---

## 4. Ingesta de hechos

### `order.completado`

1. Se consulta la orden en `orders` (`GET /internal/orders/{id}`).
2. Se expande por línea con empresa, negocio, vendedor, tienda, SKU, cantidad y
   precio unitario.
3. Se inserta en `analytics.hechos_venta` (incluye `comprador_id` y
   `orden_creada_en` para cohortes).
4. Se marca el `event_id` como procesado en la misma transacción.

### `comision.acreditada`

Inserta/actualiza `analytics.comisiones` con empresa, negocio, vendedor y
monto; sirve para separar GMV de ingreso por comisión.

### Idempotencia y resiliencia

- `event_id` en `analytics.eventos_procesados`.
- Si orders no responde, el evento falla y RabbitMQ reintenta; agotados los
  intentos → DLQ.
- El consumidor corre con contexto de servicio (`super_admin`).

---

## 5. Analítica

### Por negocio

`GET /api/v2/analytics/negocios?desde&hasta&negocio_id`

Devuelve, por negocio: órdenes, clientes únicos, ventas, comisiones, ticket
promedio y evolución mensual. Los hechos sin negocio se agrupan como
“sin negocio”.

### Empresa

`GET /api/v2/analytics/empresa?desde&hasta` consolida todos los negocios de la
empresa activa.

### Plataforma

`GET /api/v2/analytics/plataforma?desde&hasta` (solo admin) agrega todas las
empresas: GMV, órdenes, comisiones, empresas/negocios activos.

### Cohortes

`GET /api/v2/analytics/negocios/cohortes?meses=N`

Agrupa compradores por mes de primera compra y calcula retención por mes
posterior. Sirve para medir recurrencia, no para atribución publicitaria.

### CSV

Todos los endpoints de analítica aceptan `formato=csv` y responden
`text/csv` con cabecera `Content-Disposition`; sin él, JSON.

---

## 6. Reportes programados

| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/v2/analytics/reportes` | Listar reportes de la empresa |
| POST | `/api/v2/analytics/reportes` | Programar un reporte |
| PATCH | `/api/v2/analytics/reportes/{id}` | Activar/pausar |
| POST | `/api/v2/analytics/reportes/{id}/ejecutar` | Ejecutar ahora |
| GET | `/api/v2/analytics/reportes/{id}/ejecuciones` | Historial |

Validaciones:

- `tipo` ∈ `analitica`, `cohortes`.
- `formato` ∈ `json`, `csv`.
- `frecuencia` ∈ `diaria`, `semanal`, `mensual`.
- `destino` de 3–200 caracteres. Si empieza con `http(s)://`, el servicio envía
  el contenido al webhook y guarda `entrega_estado`/`entrega_error`; si no, la
  entrega queda a cargo de otro consumidor.

El worker cada **5 minutos** ejecuta hasta 20 reportes vencidos
(`proxima_ejecucion <= NOW()`), persiste el resultado en
`reporte_ejecuciones` y recalcula la próxima fecha. Los webhooks se entregan al
terminar; los demás destinos quedan para otro consumidor.

`POST /internal/analytics/reportes/ejecutar?limite=N` (1–100, default 20)
permite forzar la ejecución desde operación.

---

## 7. Seguridad y multi-tenancy

- `analitica_negocios` debe estar activa para roles de empresa; admin y
  super_admin están exentos.
- `empresa_id` sale del JWT (`x-tenant`), nunca del body.
- Un empresario solo ve su empresa; un negocio se filtra si se indica.
- `/plataforma` exige rol global admin.
- El contrato interno exige `x-internal-key` en tiempo constante.
- No se exponen datos personales: solo agregados y `comprador_id` cuando el
  endpoint lo requiere para cohortes (no se devuelve el ID en la respuesta).

---

## 8. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas HTTP + contadores de reportes.
- Logs JSON: `analytics.reportes`, `analytics.consumer`.
- Dos colas declaradas al arrancar; supervisor RabbitMQ y reintento cada 10 s.
- Worker de reportes cada 5 min, abortado en apagado ordenado.

### Diagnóstico frecuente

| Síntoma | Causa |
|---|---|
| Sin datos | `order.completado` no consumido o orders caído |
| 404 en `/api/v2/analytics/**` | El gateway no tiene destino para analytics |
| Reporte no ejecuta | `activo=false`, `proxima_ejecucion` futura o worker caído |
| Totales duplicados | `event_id` distinto en un reintento (no debería ocurrir) |
| `CAPACIDAD_NO_ACTIVA` | `analitica_negocios` desactivada en el tenant |

---

## 9. Pruebas

```bash
cd packages/analytics-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/analitica_pg.rs`: hechos, resumen por negocio, empresa, cohortes,
  reportes y ejecuciones con PostgreSQL real.
- `tests/sql/analytics.sql`: esquema mínimo.
- Tests puros de validación de fechas y CSV.

---

## 10. Referencias

- [ADR-0022](../../docs/adr/0022-analytics-service.md)
- [ADR-0021](../../docs/adr/0021-negocios-por-empresa.md)
- [Contrato público](../../docs/openapi/analytics-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/analytics-service/internal.openapi.json)
- [README de paquetes](../README.md)
