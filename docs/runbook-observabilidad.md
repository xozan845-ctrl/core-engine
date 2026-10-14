# Runbook de observabilidad

Procedimiento operativo para las alertas definidas en
[`infra/prometheus/rules/alerts.yml`](../infra/prometheus/rules/alerts.yml).
Antes de intervenir, registrar la hora, la alerta, el servicio afectado y un
`x-request-id` representativo. No copiar tokens, credenciales ni PII al ticket.

## Servicio caído

1. Confirmar en Prometheus si `up` falta para una sola instancia o para todo el job.
2. Consultar estado y reinicios del contenedor; revisar sus logs JSON desde el último arranque.
3. Probar `/health` y `/metrics` desde la red `core-engine`; si ambos fallan, revisar dependencias.
4. Escalar antes de reiniciar cuando exista pérdida repetida o corrupción de datos.

## Event loop y memoria

1. Correlacionar lag, GC, heap y uso del contenedor en los dashboards Node.js/containers.
2. Revisar tráfico, consultas lentas y operaciones síncronas durante la misma ventana.
3. Capturar evidencia antes de escalar réplicas o reiniciar; comparar con el último despliegue.
4. Si la presión sigue creciendo, limitar tráfico y escalar al responsable del servicio.

## PostgreSQL

1. Revisar conexiones activas, consultas largas, locks, cache hit y espacio disponible.
2. Identificar el servicio y despliegue que originan el cambio; no terminar sesiones a ciegas.
3. Para dead tuples, validar autovacuum y programar mantenimiento fuera del pico.
4. Escalar al responsable de datos si hay bloqueos, saturación o riesgo de capacidad.

## RabbitMQ y DLQ

1. Identificar la cola, su tasa de publicación/entrega, consumidores y antigüedad del mensaje.
2. Para una DLQ, inspeccionar `x-error`, `x-intentos`, `x-request-id` y routing key en una muestra.
3. Corregir primero la causa; pausar la reinyección si continúa generando mensajes muertos.
4. Reinyectar de forma controlada y comprobar que backlog y DLQ decrecen sin nuevos errores.

## Errores HTTP

1. Separar 5xx por servicio, ruta y estado; localizar un `x-request-id` en los logs JSON.
2. Correlacionar con latencia, base de datos, RabbitMQ y cambios del último despliegue.
3. Mitigar con rollback o reducción de tráfico cuando la tasa siga sobre el umbral.
4. Registrar causa, impacto y verificación de recuperación.

## Latencia HTTP

1. Comparar p95/p99 por servicio y ruta parametrizada con tasa y errores.
2. Revisar consultas lentas, backlog de colas, CPU, memoria y event-loop lag.
3. Aplicar la mitigación del componente limitante y confirmar recuperación durante dos ventanas.

## Disco

1. Identificar filesystem y consumidor de espacio; comprobar crecimiento y retención.
2. Rotar o archivar datos según política; no borrar volúmenes ni datos de aplicación sin respaldo.
3. Ampliar capacidad o reducir retención si el crecimiento supera el margen operativo.
