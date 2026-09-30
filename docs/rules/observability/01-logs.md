# 01 — Reglas Doradas de Observabilidad (logs y correlación)

Aplica a: logs estructurados y propagación de correlación IDs en todos los
servicios.

> **Alcance:** este documento norma los **logs estructurados** y la propagación
> de trazas (correlation IDs). Las reglas sobre **métricas de Prometheus y
> dashboards de Grafana** están en [`02-telemetria.md`](./02-telemetria.md).
> Ambos documentos son complementarios y deben aplicarse juntos.

## Reglas

| ID | Regla |
|---|---|
| R-OB-1 | **Logging estructurado (JSON)**: absolutamente todos los logs emitidos por los microservicios en entornos de staging/producción deben estar en formato estructurado (JSON). Prohibido imprimir objetos complejos usando `console.log` estándar que ensucie la salida (stdout). |
| R-OB-2 | **Correlación de trazas (distributed tracing)**: cada solicitud HTTP que ingresa al Gateway debe recibir un `x-request-id` (correlation ID) único. Este ID DEBE propagarse a lo largo de toda la cadena de microservicios. **Flujo síncrono:** el ID viaja como cabecera HTTP. **Flujo asíncrono (Outbox/RabbitMQ):** el `x-request-id` DEBE almacenarse en los metadatos del registro de la tabla `outbox` junto con el payload del evento, para que el relay lo incluya como header del mensaje de RabbitMQ. Sin este paso, la correlación de trazas se pierde para todos los flujos event-driven y el debugging distribuido se vuelve imposible. |
| R-OB-3 | **Métricas de negocio relevantes**: además de las métricas estándar de infraestructura (CPU, RAM) y latencia HTTP, se deben exponer métricas de negocio para Prometheus en `/metrics`. Los nombres deben seguir la convención `R-TM-2` (únicamente ASCII, guiones bajos). Ejemplos válidos: `core_engine_orders_created_total`, `core_engine_payments_failed_total`. Ejemplos inválidos con caracteres especiales como `órdenes_creadas_total` serán rechazados silenciosamente por Prometheus. |
| R-OB-4 | **Niveles de log adecuados**: `ERROR` = cosas que fallaron y necesitan atención inmediata (excepciones de red, caídas); `WARN` = comportamientos anómalos pero tolerables (retry alcanzado, cliente con formato raro); `INFO` = cambios de estado críticos de negocio (pedido creado, liquidación cerrada); `DEBUG` = detalles de desarrollo (cargas de configuración, respuestas detalladas) — no deben imprimirse en producción. |
| R-OB-5 | **No exponer datos sensibles en logs (sanitization)**: NUNCA se deben escribir en los logs contraseñas, tokens JWT, información de tarjetas, ni PII directo sin enmascarar. |
