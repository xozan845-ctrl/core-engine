# Reglas Doradas de Telemetría (Telemetry - Prometheus & Grafana)

1. **Exposición Universal de Métricas**: Todo microservicio, sin excepción, DEBE exponer un endpoint `/metrics` en texto plano compatible con Prometheus. Ningún servicio puede ir a producción si no está siendo "scrapeado" (recolectado) activamente.
2. **Convención de Nombres de Métricas**: 
   - Deben usar el formato `[namespace]_[subsistema]_[nombre]_[unidad]` con **únicamente guiones bajos** (`_`). Prometheus no permite guiones (`-`) en nombres de métricas.
   - Ejemplo correcto: `core_engine_orders_created_total`, `core_engine_http_request_duration_seconds`.
   - Prohibido usar nombres genéricos como `requests` o `errores`.
3. **Uso Adecuado de Etiquetas (Labels / Tags)**:
   - Los labels deben ser acotados y de **baja cardinalidad**. Nunca incluir IDs únicos (ej. `user_id`, `order_id`) como etiqueta en una métrica de Prometheus, ya que esto explotaría el uso de RAM del servidor de telemetría.
   - Etiquetas permitidas (en concordancia con las ya definidas en el sistema): `metodo`, `estado`, `servicio`, `ruta` (parametrizada, no la URL en crudo, p.ej. `/api/v1/orders/:id` no `/api/v1/orders/abc-123`). Nunca mezclar inglés y español en los label names de una misma métrica.
4. **Métricas RED Obligatorias**: Todo servicio debe implementar y registrar invariablemente las métricas RED (Rate, Errors, Duration):
   - **Rate (Tasa)**: Cantidad de peticiones por segundo.
   - **Errors (Errores)**: Dos señales distintas: (1) **5xx** — errores de infraestructura/servicio (umbral crítico, indica fallo del sistema). (2) **4xx relevantes** — tasa de 401/403 (posible ataque de credenciales) y tasa de 409 (colisión de stock, conflicto de negocio). Ambas se deben registrar como métricas separadas con el label `estado` para no mezclar errores de sistema con errores de negocio en las mismas alertas.
   - **Duration (Duración)**: Histograma de latencia para medir percentiles (p95, p99).
5. **Métricas de Infraestructura y Broker**: Es obligatorio recolectar métricas del estado del Event Loop de Node.js, así como del tamaño de las colas (Backlog) de RabbitMQ y DLQ. La alerta crítica de DLQ NO debe dispararse por la mera existencia de mensajes (ya que mensajes muertos son comportamiento normal de diseño, ver `architecture.md` regla 4). La alerta debe configurarse sobre: (a) **crecimiento sostenido** de la DLQ en los últimos N minutos, o (b) **mensajes sin procesar/remediar** pasadas X horas del SLA de respuesta operativa definido por el equipo.
6. **Inmutabilidad de Dashboards**: Los tableros en Grafana deben configurarse como **código (Dashboard as Code)** y aprovisionarse automáticamente desde el repositorio (`infra/grafana/provisioning`). No se permite crear tableros vitales manualmente en la interfaz web sin respaldarlos en el repositorio, previniendo su pérdida.
7. **Alertas Accionables**: Las alertas configuradas a partir de métricas deben ser descriptivas y sugerir un plan de acción (Playbook). Una alerta de "Alta Latencia" debe indicar si el problema reside en base de datos, colas o CPU, en lugar de solo arrojar el aviso genérico.
