# 02 — Excepción OLAP: Inteligencia de Mercado

Aplica a: `packages/market-intelligence-service` — el microservicio tiene
naturaleza analítica (OLAP / data-intensive): procesamiento de grandes
volúmenes, agregaciones complejas y análisis predictivos. **Es la única
excepción autorizada** a varias reglas transaccionales globales y se rige por
los siguientes principios de data engineering.

## Reglas

| ID | Regla |
|---|---|
| R-MI-1 | **Excepción OLAP y CQRS (write/read models)**: a diferencia de los microservicios transaccionales (OLTP) donde prima el CRUD (ver `R-AR-1`, `../architecture/01-arquitectura.md`), Inteligencia de Mercado implementa un modelo **CQRS analítico**. <br>• **Write path (ingesta):** los eventos (hechos) se insertan de forma atómica en tablas base crudas (ej. `hechos_venta`). <br>• **Read path (consumo):** las lecturas nunca se hacen sobre los hechos crudos; se deben utilizar siempre vistas materializadas (ej. `rendimiento_vendedor`, `puntos_calor`) o pipelines de agregación. |
| R-MI-2 | **Geospatial analytics (PostGIS obligatorio)**: para habilitar analítica de mapas de calor avanzados, clustering espacial y análisis por proximidad. <br>• **Prohibición de `lat`/`lng` aislados:** todo dato espacial debe persistirse usando el tipo `GEOMETRY(Point, 4326)` nativo de PostGIS (ej. `ST_SetSRID(ST_MakePoint(lng, lat), 4326)`). <br>• **Índices GIST:** las tablas con datos espaciales deben tener índices GIST obligatorios (`USING GIST (geom)`) para optimizar queries geográficas (`ST_DWithin`, `ST_Contains`). |
| R-MI-3 | **Data quality y schema contracts (runtime)**: los eventos transaccionales que llegan por el broker pueden sufrir evolución de esquema (schema drift). Todo dato ingerido debe ser validado en **tiempo de ejecución (runtime)** contra un data contract (ej. JSON Schema) estricto (data quality layer) antes de persistirse. Eventos corruptos o que no cumplan el SLA de calidad (ej. coordenadas inválidas, montos negativos) no deben bloquear la cola; se rechazan o envían a una tabla de anomalías (`invalid_events`) reportando la métrica respectiva en Prometheus. |
| R-MI-4 | **Evolución de streaming (RabbitMQ a Kafka/ksqlDB)** — política aplicable para la Fase 2 del roadmap. <br>• **Coexistencia de brokers:** RabbitMQ se mantiene como el bus de eventos **transaccional y orquestador** (`R-AR-2`, `../architecture/01-arquitectura.md`). <br>• La adopción de Apache Kafka / ksqlDB en este servicio está autorizada **estrictamente como motor de streaming analítico** (arquitectura Lambda/Kappa). Los eventos transaccionales de RabbitMQ pueden ser puenteados hacia Kafka para su materialización en tiempo real (real-time materialized views), pero Kafka **no sustituirá** a RabbitMQ para la orquestación de procesos de negocio críticos (ej. sagas). |
| R-MI-5 | **Modelos predictivos y feature store**: <br>• **Cómputo asíncrono:** los modelos de machine learning (forecasting, churn, detección de anomalías) no deben calcular sus características (features) al vuelo durante la petición HTTP. <br>• **Feature store:** las variables derivadas (ej. rotación de inventario 30d, densidad de zona) deben pre-calcularse asíncronamente y almacenarse en una tabla/store de características (`feature_store`) para garantizar latencias de lectura menores a 100 ms. |
