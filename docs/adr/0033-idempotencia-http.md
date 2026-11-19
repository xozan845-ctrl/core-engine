# ADR-0033 — Idempotencia HTTP en mutaciones críticas

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0013](./0013-orders-service-en-rust.md),
  [ADR-0027](./0027-pagos-proveedor-reembolsos.md)

## Contexto

Un reintento de red, un doble clic o un cliente que repite `POST /orders` podía
crear dos órdenes, reservar stock dos veces o duplicar un pago. El versionado
optimista del event store protege la concurrencia del agregado, pero no la
intención del cliente.

## Decisión

1. **Cabecera `Idempotency-Key`.** Es opcional y opaca; máximo 255 caracteres.
   El gateway la reenvía al servicio de destino.
2. **Alcance por actor y clave.** La tabla
   `orders.idempotencia_http` usa `(actor_id, clave)` como PK y guarda método,
   ruta, hash del cuerpo, estado, status, content-type y respuesta.
3. **Replay de la respuesta.** Si la clave ya está `completada` y coinciden
   método, ruta y hash, se devuelve la respuesta 2xx original con
   `Idempotency-Replayed: true`.
4. **Clave reutilizada.** Si la misma clave llega con otro método, ruta o
   cuerpo, se responde `422 IDEMPOTENCIA_CLAVE_REUTILIZADA`.
5. **Solicitud concurrente.** Si la clave está `en_proceso`, se responde
   `409 IDEMPOTENCIA_EN_PROCESO`; no se ejecuta el handler.
6. **Errores no se cachean.** Las respuestas no 2xx eliminan el registro para
   que el cliente pueda corregir y reintentar.
7. **Limpieza.** Un worker horario borra respuestas completadas con más de
   7 días y registros `en_proceso` con más de 15 minutos.
8. **Cobertura.** El middleware aplica a `POST/PUT/PATCH/DELETE` bajo
   `/api/v2/**` cuando la cabecera está presente. Las rutas `/internal/**` no
   participan.

## Consecuencias

- **Positivas:** reintentos seguros en órdenes, pagos, devoluciones y
  cotizaciones; el cliente puede reintentar sin duplicar efectos.
- **Contrato:** `Idempotency-Key` documentada en OpenAPI para las mutaciones
  críticas; nuevos códigos `IDEMPOTENCIA_EN_PROCESO` (409) y
  `IDEMPOTENCIA_CLAVE_REUTILIZADA` (422).
- **Datos:** migración `99_idempotencia_http.sql`; sin RLS porque la tabla solo
  la usa orders-service con el rol de aplicación.
- **Operación:** el middleware guarda la respuesta completa en memoria y en
  `bytea`; las respuestas de orders son JSON pequeños.
- **Pendiente:** extender la misma tabla o un servicio común a otros
  servicios; TTL configurable; política de espera en lugar de 409 para
  solicitudes concurrentes.

## Criterios de aceptación

- Dos `POST /orders` con la misma clave y cuerpo crean una sola orden.
- El reintento devuelve el mismo `id` y `Idempotency-Replayed: true`.
- La misma clave con otro cuerpo responde 422.
- La misma clave con otro actor no colisiona.
- Un error de validación no queda cacheado: el siguiente intento se ejecuta.
- La confirmación de un pago es idempotente con la misma clave.
