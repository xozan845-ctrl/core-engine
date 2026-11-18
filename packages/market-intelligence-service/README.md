# market-intelligence-service · inteligencia de mercado

`market-intelligence-service` convierte ventas, comisiones, stock y geolocalización
en **inteligencia accionable**: mapas de calor, rendimiento de vendedores,
cobertura por zona, demanda, tendencias, forecast, anomalías, score predictivo y
calidad del dato.

Es la capacidad `inteligencia_mercado`. Usa PostGIS para celdas geográficas y
un *feature store* para las predicciones.

- [ADR-0009 · market-intelligence-service en Rust](../../docs/adr/0009-market-intelligence-service-en-rust.md)
- Contrato: [`docs/openapi/market-intelligence-service/`](../../docs/openapi/market-intelligence-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `market-intelligence-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest`, PostGIS |
| Puerto | `3009` (`PORT`) |
| Esquema | `intelligence` |
| Outbox | `intelligence.outbox` (infraestructura; sin eventos de negocio hoy) |
| Capacidad | `inteligencia_mercado` |
| Consume | `venta.geolocalizada` (cola `intelligence.venta_geo`), `comision.acreditada` (cola `intelligence.order_completado`), `stock.updated` (cola `intelligence.stock_updated`) |
| Workers | features cada 5 min; refresco de vistas materializadas cada 30 s |
| Entornos | dev, stage y producción (según compose) |
| Expuesto al host | No |

> El `README.md` raíz describe este servicio como “solo dev”, pero
> `docker-compose.yml` y `docker-compose.stage.yml` lo despliegan. La fuente
> operativa es el compose. Además, el gateway no incluye un destino para
> `/api/v2/inteligencia/**`… **sí lo incluye** (`intelligence`), por lo que la
> superficie es alcanzable si el servicio está arriba.

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL + PostGIS |
| `RABBITMQ_URL` | **Sí** | — | Consumidor |
| `PORT` | No | `3009` | Puerto |
| `SERVICIO` | No | `market-intelligence-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Internos |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Capacidades |
| `OUTBOX_TABLA` | No | `outbox` | No usado para publicar |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `inteligencia/mod.rs` | Consultas analíticas, filtros, forecast, anomalías, score y calidad |
| `agregador.rs` | Upserts atómicos de ventas, comisiones y stock en el esquema |
| `consumidor.rs` | Consumo multi-cola e idempotencia |
| `calidad.rs` | Contrato de datos, score de completitud y `invalid_events` |
| `tareas.rs` | Cómputo de features y refresco de vistas materializadas |
| `http/inteligencia.rs` | Endpoints y validación de filtros |
| `auth.rs` | Capacidad `inteligencia_mercado`, roles y `/me` |

---

## 3. Modelo de datos

| Tabla/vista | Contenido |
|---|---|
| `intelligence.hechos_venta` | Venta geolocalizada: orden, vendedor, SKU, monto, `lat`/`lng`, fecha, empresa |
| `intelligence.rendimiento_vendedor` | Pedidos, entregados, tasa de efectividad |
| `intelligence.cobertura_zona` | Celdas lat/lng, ventas, monto, vendedores, `bounding_box` (PostGIS) |
| `intelligence.metricas_producto` | Demanda por SKU |
| `intelligence.puntos_calor` | Puntos agregados para el mapa de calor |
| `intelligence.tendencias_temporales` | Serie por periodo |
| `intelligence.feature_store` | Features por SKU, vendedor y zona |
| `intelligence.invalid_events` | Eventos rechazados y sus errores |
| `intelligence.eventos_procesados` | Idempotencia |
| `intelligence.mv_resumen_ejecutivo`, `mv_tendencias` | Vistas materializadas |
| `intelligence.vw_anomalias_recientes` | Z-score por SKU sobre 30 días |

### Invariantes

- Una venta sin SKUs es inválida (`VENTA_SIN_SKUS`).
- `event_id` único: la agregación es idempotente.
- Las celdas geográficas se agrupan por lat/lng redondeadas; la cobertura usa
  `ST_AsGeoJSON` del *bounding box*.
- `lat`/`lng` son opcionales: una venta sin geolocalización no entra al mapa,
  pero sí a los agregados.
- Los filtros de fecha deben ser ISO; `vendedor_id` debe ser UUID.
- El feature store se actualiza con `ON CONFLICT` (upsert), nunca duplica.

---

## 4. Ingesta

### `venta.geolocalizada`

Evento explícito de venta con geolocalización. El agregador:

1. Valida el contrato en `calidad.rs` (campos obligatorios, tipos, rangos).
2. Si es inválido, lo persiste en `invalid_events`, incrementa
   `core_engine_intelligence_invalid_events_total` y lo descarta.
3. Si es válido, upserta en una transacción:
   `hechos_venta`, `puntos_calor`, `rendimiento_vendedor`,
   `cobertura_zona`, `metricas_producto`, `tendencias_temporales`.
4. Marca `eventos_procesados`.

### `comision.acreditada`

Actualiza rendimiento del vendedor (pedidos entregados/efectividad) y las
métricas de comisión.

### `stock.updated`

Actualiza `metricas_producto` con el stock restante y el nombre del SKU.

### Calidad de datos

`calidad.rs` define el contrato del evento:

- Campos obligatorios: orden, vendedor, SKUs, monto, fecha.
- 10 campos opcionales (lat, lng, precisión, velocidad, rumbo, tipo de
  actividad, resultado de visita, distancia, edad, género).
- `completeness_score = rellenos / 10`.
- Tiers: **HIGH** `> 0.9`, **MEDIUM** `> 0.5`, **LOW** en el resto.
- Métricas por tier y contador de inválidos.

---

## 5. Analítica expuesta

| Método | Ruta | Contenido |
|---|---|---|
| GET | `/api/v2/inteligencia/mapa-calor` | Puntos/celdas de calor con filtros |
| GET | `/api/v2/inteligencia/me/mapa-calor` | Mapa del vendedor autenticado |
| GET | `/api/v2/inteligencia/rendimiento` | Pedidos, entregados y efectividad por vendedor |
| GET | `/api/v2/inteligencia/me/rendimiento` | Rendimiento propio |
| GET | `/api/v2/inteligencia/cobertura` | Celdas y GeoJSON de cobertura |
| GET | `/api/v2/inteligencia/demanda` | Demanda por SKU |
| GET | `/api/v2/inteligencia/tendencias` | Serie temporal |
| GET | `/api/v2/inteligencia/resumen` | Resumen ejecutivo |
| POST | `/api/v2/inteligencia/ventas/registrar` | Ingesta manual de una venta |
| GET | `/api/v2/inteligencia/forecast/demanda?sku&dias` | Forecast |
| GET | `/api/v2/inteligencia/anomalias` | Ventas anómalas por Z-score |
| GET | `/api/v2/inteligencia/vendedores/{id}/score` | Score del vendedor (admin) |
| GET | `/api/v2/inteligencia/me/score` | Score propio |
| GET | `/api/v2/inteligencia/calidad?horas=N` | Calidad del pipeline |

### Forecast

- Usa `ventas_avg_7d` y `ventas_std_7d` del feature store por SKU.
- Proyección = `avg × días`.
- Intervalo = `± std × √días` (aprox. independencia diaria), acotado a ≥ 0.
- Metodología declarada: **media móvil ponderada 7d**. No es un modelo
  estacional; es una línea base explicable.

### Anomalías

`vw_anomalias_recientes` calcula por SKU, sobre 30 días:

```
z = |monto − media| / stddev_poblacional
```

El endpoint devuelve orden, SKU, vendedor, monto, coordenadas, fecha y
`z_score` redondeado a 3 decimales. Si `stddev = 0`, no hay anomalía.

### Score de vendedor

Desde `feature_store`:

- `tasa_efectividad` (entregados / pedidos).
- `churn_risk_score`: 1.0 si efectividad < 0.5; 0.5 si < 0.75; 0.0 si ≥ 0.75.
- Nivel: `ALTO` (≥1.0), `MEDIO` (≥0.5), `BAJO`.

Es un indicador de riesgo operativo, no una predicción de abandono entrenada.

---

## 6. Tareas de fondo

| Tarea | Frecuencia | Qué hace |
|---|---|---|
| Features | 5 min | `ventas_avg_7d`, `ventas_std_7d` por SKU; efectividad/churn por vendedor; densidad por zona |
| Vistas | 30 s | `REFRESH MATERIALIZED VIEW CONCURRENTLY` de `mv_resumen_ejecutivo` y `mv_tendencias` |

- Las tres ramas de features corren aunque una falle (`tokio::join!`); se
  registra el primer error.
- El refresco de vistas registra el gauge
  `core_engine_intelligence_view_refresh_duration_seconds`.
- Ambas tareas se abortan en el apagado ordenado.

---

## 7. Seguridad y multi-tenancy

- Capacidad `inteligencia_mercado` validada contra identity.
- Rutas globales (`mapa-calor`, `rendimiento`, `demanda`, `cobertura`,
  `tendencias`, `resumen`, `forecast`, `anomalías`, score por id) exigen
  `super_admin`/`admin` según gateway.
- Rutas `/me` usan `x-user-personal`; sin ficha de personal → 401.
- La ingesta manual permite vendedor/empresario/admin.
- Los filtros por empresa se derivan del tenant cuando aplica.
- No se devuelven datos personales, solo agregados y coordenadas de venta.

---

## 8. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas de negocio:
  - `core_engine_intelligence_invalid_events_total`
  - `core_engine_intelligence_data_quality_{high,medium,low}_total`
  - `core_engine_intelligence_view_refresh_duration_seconds`
- Logs JSON: `inteligencia.agregador`, `inteligencia.calidad`,
  `inteligencia.features`, `inteligencia.vistas`.
- Tres colas declaradas; supervisor RabbitMQ y reintento cada 10 s.
- PostGIS es requisito: sin la extensión, cobertura/materialized views fallan.

### Diagnóstico frecuente

| Síntoma | Causa |
|---|---|
| `VENTA_SIN_SKUS` | Evento mal formado (500 en el consumidor) |
| Sin puntos en el mapa | Ventas sin `lat`/`lng` |
| Forecast en 0 | Feature store vacío o worker caído |
| Anomalías vacías | `stddev = 0` o sin ventas en 30 días |
| Refresco lento | Vistas materializadas grandes; revisar gauge |

---

## 9. Pruebas

```bash
cd packages/market-intelligence-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/inteligencia_pg.rs`: agregación, cobertura, demanda, forecast,
  anomalías y score con PostgreSQL/PostGIS real.
- Tests puros de calidad de datos (tiers y errores) y parseo de filtros.

---

## 10. Referencias

- [ADR-0009](../../docs/adr/0009-market-intelligence-service-en-rust.md)
- [Contrato público](../../docs/openapi/market-intelligence-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/market-intelligence-service/internal.openapi.json)
- [README de paquetes](../README.md)
