# 02 — Reglas Doradas de Telemetría (Prometheus y Grafana)

Aplica a: `/metrics`, naming y labels de métricas, métricas RED, métricas de
infraestructura/broker, dashboards y alertas.

> Complementa [`01-logs.md`](./01-logs.md) (logs y correlación de trazas):
> ambos documentos se aplican juntos.

## Reglas

| ID | Regla |
|---|---|
| R-TM-1 | **Exposición universal de métricas**: todo microservicio, sin excepción, DEBE exponer un endpoint `/metrics` en texto plano compatible con Prometheus. Ningún servicio puede ir a producción si no está siendo "scrapeado" (recolectado) activamente. |
| R-TM-2 | **Convención de nombres de métricas**: deben usar el formato `[namespace]_[subsistema]_[nombre]_[unidad]` con **únicamente guiones bajos** (`_`). Prometheus no permite guiones (`-`) en nombres de métricas. Ejemplo correcto: `core_engine_orders_created_total`, `core_engine_http_request_duration_seconds`. Prohibido usar nombres genéricos como `requests` o `errores`. |
| R-TM-3 | **Uso adecuado de etiquetas (labels/tags)**: los labels deben ser acotados y de **baja cardinalidad**. Nunca incluir IDs únicos (ej. `user_id`, `order_id`) como etiqueta en una métrica de Prometheus, ya que esto explotaría el uso de RAM del servidor de telemetría. Etiquetas permitidas (en concordancia con las ya definidas en el sistema): `metodo`, `estado`, `servicio`, `ruta` (parametrizada, no la URL en crudo; p.ej. `/api/v1/orders/:id`, no `/api/v1/orders/abc-123`). Nunca mezclar inglés y español en los label names de una misma métrica. |
| R-TM-4 | **Métricas RED obligatorias**: todo servicio debe implementar y registrar invariablemente las métricas RED (rate, errors, duration). <br>• **Rate (tasa):** cantidad de peticiones por segundo. <br>• **Errors (errores):** dos señales distintas: (1) **5xx** — errores de infraestructura/servicio (umbral crítico, indica fallo del sistema); (2) **4xx relevantes** — tasa de 401/403 (posible ataque de credenciales) y tasa de 409 (colisión de stock, conflicto de negocio). Ambas se registran como métricas separadas con el label `estado` para no mezclar errores de sistema con errores de negocio en las mismas alertas. <br>• **Duration (duración):** histograma de latencia para medir percentiles (p95, p99). |
| R-TM-5 | **Métricas de infraestructura y broker**: es obligatorio recolectar métricas del estado del event loop de Node.js, así como del tamaño de las colas (backlog) de RabbitMQ y DLQ. La alerta crítica de DLQ NO debe dispararse por la mera existencia de mensajes (los mensajes muertos son comportamiento normal de diseño, ver `R-AR-4`, `../architecture/01-arquitectura.md`). La alerta debe configurarse sobre: (a) **crecimiento sostenido** de la DLQ en los últimos N minutos, o (b) **mensajes sin procesar/remediar** pasadas X horas del SLA de respuesta operativa definido por el equipo. |
| R-TM-6 | **Inmutabilidad de dashboards**: los tableros en Grafana deben configurarse como **código (dashboard as code)** y aprovisionarse automáticamente desde el repositorio (`infra/grafana/provisioning`). No se permite crear tableros vitales manualmente en la interfaz web sin respaldarlos en el repositorio, previniendo su pérdida. |
| R-TM-7 | **Alertas accionables**: las alertas configuradas a partir de métricas deben ser descriptivas y sugerir un plan de acción (playbook). Una alerta de "alta latencia" debe indicar si el problema reside en base de datos, colas o CPU, en lugar de solo arrojar el aviso genérico. |
