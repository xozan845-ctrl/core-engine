# Core Engine · notifications-service

Notificaciones transaccionales por correo del ciclo del pedido (ADR-0003).

- Consume los eventos existentes del bus (`payment.procesado`,
  `shipment.started`, `order.status.updated`, `order.completado`,
  `devolucion.solicitada`, `stock.fallido`); no publica eventos nuevos.
- Resuelve comprador, vendedor y detalle del pedido por las APIs internas de
  `identity` y `orders`; no lee esquemas ajenos.
- Idempotencia por `event_id` en `notifications.eventos_procesados`; los
  reintentos y la DLQ son los de `RabbitService`.
- Usa `@core/shared/email`: en dev/test `EMAIL_MODE=log`; en stage/prod SMTP.
- No expone HTTP público: solo `/health`, `/metrics` y `/docs`.

Puerto interno: **3012**.
