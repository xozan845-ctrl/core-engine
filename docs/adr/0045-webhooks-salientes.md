# ADR-0045 — Webhooks salientes por tenant

- **Fecha:** 2026-11-20
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0006](./0006-notifications-service-en-rust.md),
  [ADR-0043](./0043-cierre-p0-retail-flota.md)

## Contexto

La integración con ERP/CRM/e-commerce externos necesitaba un mecanismo de
eventos salientes. El proyecto ya tenía bus interno (RabbitMQ + outbox) y dos
webhooks puntuales: entrega de reportes de analytics y recepción de pagos. No
existía una suscripción genérica por tenant, firmas, reintentos ni trazabilidad
de entregas.

## Decisión

1. **Modelo de suscripción.**
   - `notifications.webhooks` guarda `empresa_id`, URL, descripción, eventos
     suscritos (vacío = todos), secreto y estado.
   - `notifications.webhook_entregas` registra una fila por
     `(webhook, event_id)` con estado, intentos, próximo intento, error y
     respuesta HTTP. La clave única hace la entrega idempotente.
2. **Consumo sin acoplar el correo.**
   - `notifications-service` declara una cola propia `notifications.webhooks`
     enlazada con `#` al exchange de eventos; así recibe todos los eventos sin
     duplicar ni alterar la cola de correo `notifications.todos`.
3. **Entrega y firma.**
   - El cuerpo es el evento JSON completo. Se envía con
     `x-core-event`, `x-core-delivery` y
     `x-core-signature: sha256=<HMAC-SHA256>` usando el secreto del webhook.
   - El secreto se muestra al crear o rotar; el listado lo oculta.
4. **Reintentos.**
   - El primer intento es inmediato. Un worker cada
     `WEBHOOK_REINTENTO_SEGUNDOS` reintenta fallos con backoff exponencial
     (30 s, 60 s, …) hasta `MAX_INTENTOS = 5`.
   - Un fallo de webhook no bloquea la cola de correo ni el evento de negocio.
5. **Administración.**
   - API admin: `GET/POST /api/v2/admin/notifications/webhooks`,
     `PATCH/DELETE /{id}`, `POST /{id}/rotar-secreto`,
     `GET /{id}/entregas` y `POST /entregas/{id}/reintentar`.
   - El tenant se toma de `x-tenant` o de `empresa_id`; un admin global debe
     indicarlo explícitamente.
6. **Seguridad y ciclo de vida.**
   - En producción `WEBHOOK_HTTPS_OBLIGATORIO=true` exige HTTPS.
   - La exportación/purga del tenant incluye webhooks, entregas y plantillas.
   - La validación de IP privada/DNS rebinding queda como endurecimiento
     posterior (ver backlog); el alta es una operación de administrador.

## Consecuencias

- **Positivas:** cualquier tenant puede integrar su ERP/e-commerce con un
  contrato estable, firma verificable, log de entregas y reintentos.
- **Semántica:** la entrega es *at-least-once*; el receptor debe deduplicar por
  `event_id`. No se garantiza orden entre eventos.
- **Operación:** los secretos se guardan en la tabla para poder firmar; el
  acceso a `notifications.webhooks` debe restringirse al rol de servicio.
- **Límite:** no hay transformación de payload por suscripción ni filtros por
  contenido; se envía el evento completo.

## Criterios de aceptación

- Un evento de un tenant suscrito llega a su URL con firma válida.
- Reenviar el mismo `event_id` no duplica la entrega.
- Un 500 se reintenta y queda `entregado` cuando el receptor responde 2xx.
- El listado no expone el secreto y la rotación invalida el anterior.
- La purga del tenant borra sus webhooks y entregas.
