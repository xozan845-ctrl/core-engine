# Reglas Doradas de Observabilidad (Observability)

> **Alcance:** Este documento norma los **logs estructurados** y la propagación de trazas (Correlation IDs). Las reglas sobre **métricas de Prometheus y dashboards de Grafana** están en [`telemetry.md`](./telemetry.md). Ambos documentos son complementarios y deben aplicarse juntos.

1. **Logging Estructurado (JSON)**: Absolutamente todos los logs emitidos por los microservicios en entornos de staging/producción deben estar en formato estructurado (JSON). Prohibido imprimir objetos complejos usando `console.log` estándar que ensucie la salida (Stdout).
2. **Correlación de Trazas (Distributed Tracing)**: Cada solicitud HTTP que ingresa al Gateway debe recibir un `x-request-id` (Correlation ID) único. Este ID DEBE propagarse a lo largo de toda la cadena de microservicios. **Flujo síncrono:** el ID viaja como cabecera HTTP. **Flujo asíncrono (Outbox/RabbitMQ):** el `x-request-id` DEBE almacenarse en los metadatos del registro de la tabla `outbox` junto con el payload del evento, para que el relay lo incluya como header del mensaje de RabbitMQ. Sin este paso, la correlación de trazas se pierde para todos los flujos event-driven y el debugging distribuido se vuelve imposible.
3. **Métricas de Negocio Relevantes**: Además de las métricas estándar de infraestructura (CPU, RAM) y latencia HTTP, se deben exponer métricas de negocio para Prometheus en `/metrics`. Los nombres deben seguir la convención de `telemetry.md` (únicamente ASCII, guiones bajos). Ejemplos válidos: `core_engine_orders_created_total`, `core_engine_payments_failed_total`. Ejemplos inválidos con caracteres especiales como `órdenes_creadas_total` serán rechazados silenciosamente por Prometheus.
4. **Niveles de Log Adecuados**: 
   - `ERROR`: Cosas que fallaron y necesitan atención inmediata (excepciones de red, caídas).
   - `WARN`: Comportamientos anómalos pero tolerables (retry alcanzado, cliente con formato raro).
   - `INFO`: Cambios de estado críticos de negocio (pedido creado, liquidación cerrada).
   - `DEBUG`: Detalles de desarrollo (cargas de configuración, respuestas detalladas). No deben imprimirse en producción.
5. **No Exponer Datos Sensibles en Logs (Sanitization)**: NUNCA se deben escribir en los logs contraseñas, tokens JWT, información de tarjetas, ni PII directo sin enmascarar.
