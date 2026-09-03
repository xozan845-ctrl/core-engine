# Reglas Doradas de Inteligencia de Mercado (Market Intelligence)

El microservicio `market-intelligence-service` tiene una naturaleza fundamentalmente analítica (OLAP / Data-intensive), orientada al procesamiento de grandes volúmenes de datos, agregaciones complejas y análisis predictivos. 

Debido a esto, **este servicio es la única excepción autorizada** a varias reglas transaccionales globales, rigiéndose por los siguientes principios de Data Engineering:

## 1. Excepción OLAP y CQRS (Write/Read Models)
A diferencia de los microservicios transaccionales (OLTP) donde prima el CRUD (ver `architecture.md` Regla 1), Inteligencia de Mercado implementa un modelo **CQRS Analítico**.
- **Write Path (Ingesta):** Los eventos (hechos) se insertan de forma atómica en tablas base crudas (ej. `hechos_venta`).
- **Read Path (Consumo):** Las lecturas nunca se hacen sobre los hechos crudos. Se deben utilizar siempre vistas materializadas (ej. `rendimiento_vendedor`, `puntos_calor`) o pipelines de agregación. 

## 2. Geospatial Analytics (PostGIS Obligatorio)
Para habilitar analítica de mapas de calor avanzados, clustering espacial y análisis por proximidad:
- **Prohibición de `lat`/`lng` aislados:** Todo dato espacial debe persistirse usando el tipo `GEOMETRY(Point, 4326)` nativo de PostGIS (ej. `ST_SetSRID(ST_MakePoint(lng, lat), 4326)`).
- **Índices GIST:** Las tablas con datos espaciales deben tener índices GIST obligatorios (`USING GIST (geom)`) para optimizar queries geográficas (`ST_DWithin`, `ST_Contains`).

## 3. Data Quality y Schema Contracts (Runtime)
Los eventos transaccionales que llegan por el broker pueden sufrir evolución de esquema (schema drift).
- Todo dato ingerido debe ser validado en **tiempo de ejecución (runtime)** contra un Data Contract (ej. JSON Schema) estricto (Data Quality Layer) antes de persistirse.
- Eventos corruptos o que no cumplan el SLA de calidad (ej. coordenadas inválidas, montos negativos) no deben bloquear la cola; se rechazan o envían a una tabla de anomalías (`invalid_events`) reportando la métrica respectiva en Prometheus.

## 4. Evolución de Streaming (RabbitMQ a Kafka/ksqlDB)
*(Política aplicable para la Fase 2 del Roadmap)*
- **Coexistencia de Brokers:** RabbitMQ se mantiene como el bus de eventos **transaccional y orquestador** (`architecture.md` Regla 2). 
- La adopción de Apache Kafka / ksqlDB en este servicio está autorizada **estrictamente como motor de streaming analítico** (Lambda/Kappa architecture). Los eventos transaccionales de RabbitMQ pueden ser puenteados hacia Kafka para su materialización en tiempo real (Real-Time Materialized Views), pero Kafka no sustituirá a RabbitMQ para la orquestación de procesos de negocio críticos (ej. sagas).

## 5. Modelos Predictivos y Feature Store
- **Cómputo Asíncrono:** Los modelos de Machine Learning (forecasting, churn, detección de anomalías) no deben calcular sus características (features) al vuelo durante la petición HTTP.
- **Feature Store:** Las variables derivadas (ej. rotación de inventario 30d, densidad de zona) deben pre-calcularse asíncronamente y almacenarse en una tabla/store de características (`feature_store`) para garantizar latencias de lectura menores a 100ms.
