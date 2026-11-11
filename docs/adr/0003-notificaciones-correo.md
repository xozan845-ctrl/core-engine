# ADR-0003: Notificaciones transaccionales y correo

- **Fecha**: 2026-11-11
- **Estado**: Aceptado — fases 1-5 completas (2026-11-11)
- **Ámbito**: correo, notificaciones del pedido, reset de contraseña, secretos

## Contexto

- `POST /api/v2/auth/restablecer-contrasena` es un **stub**: verifica el correo y
  devuelve `{ ok: true }`, pero no envía nada (`auth.controller.ts`).
- `logistics.pedidos` llama a `notificar('email', ...)` pero solo escribe un log
  `notificacion_enviada`; no hay proveedor de correo ni servicio de notificaciones.
- Los contratos ya nombran a "Notificaciones" como consumidor, y los eventos
  `payment.procesado`, `shipment.started`, `order.completado` y
  `order.status.updated` ya existen en el bus.
- No hay dependencia de correo, ni ADR, ni variables SMTP en los `.env*.example`.

## Decisión

### 1. Proveedor intercambiable en `@core/shared/email`

- `EmailProvider` con `send({ to, subject, text, html })`.
- `LogEmailProvider` para dev/test: no envía; registra el destinatario enmascarado.
- `SmtpEmailProvider` con `nodemailer` para stage/prod; compatible con Supabase
  Auth SMTP o cualquier relay.
- Configuración: `EMAIL_MODE=log|smtp`, `EMAIL_FROM`, `SMTP_HOST`, `SMTP_PORT`,
  `SMTP_USER`, `SMTP_PASSWORD`. Los secretos viven solo en el entorno
  (R-DS-1/R-SB-5); nunca en el repositorio.

### 2. `notifications-service` (14.º workspace) para el ciclo del pedido

- Consume los **eventos existentes**; no crea eventos nuevos.
- Idempotencia por `event_id` en `eventos_procesados` y DLQ/backoff reutilizando
  el patrón de `RabbitModule` y `OutboxModule`.
- Resuelve el destinatario por la API interna de identity
  (`/internal/usuarios/:id` y partes) y el detalle por `/internal/orders/:id`;
  no lee esquemas ajenos.
- No expone HTTP público: solo `/health`, `/metrics` e `/internal/*`.
- Plantillas: pedido pagado, envío iniciado, pedido entregado, cancelación y
  devolución.

### 3. `identity` envía autenticación e invitaciones con el mismo provider

- Tabla `identity.password_reset_tokens` (hash, expira, usado, creado).
- `POST /auth/restablecer-contrasena` genera y envía el enlace.
- `POST /auth/restablecer-contrasena/confirmar` consume el token de un solo uso
  y actualiza la contraseña.
- No revela si el correo existe; expiración corta y rate limit.
- Las invitaciones de personal usan el mismo adapter; no se duplica el envío.

### 4. Sin correos reales en tests

- `EMAIL_MODE=log` en unit, integración y E2E; un fake captura el mensaje.
- El E2E valida el flujo sin SMTP y sin credenciales.

## Fases de implementación

| Fase | Alcance | PR |
|---|---|---|
| 1 | `@core/shared/email` + adapters + tests | #536 |
| 2 | Reset de contraseña en identity (migración, endpoints, tests) | #544 |
| 3 | `notifications-service` + plantillas + consumidor | #545 |
| 4 | Invitaciones por correo con el mismo adapter | #546 |
| 5 | E2E del reset con SMTP real (Mailpit) | #547 |

Cada fase es un PR atómico y no adelanta la siguiente. La migración de reset se
registra en `scripts/config/db-migrations.json` (R-DB-6) y la tabla lleva RLS
(R-SB-2).

## Consecuencias

### Positivas

- El reset deja de ser un stub y el usuario puede recuperar su cuenta.
- Los correos del pedido se desacoplan de la saga: un fallo de SMTP no bloquea
  la orden.
- El proveedor es intercambiable; no se ata el dominio a Supabase.

### Costes y riesgos

- +1 workspace, `nodemailer` en shared y variables SMTP en el despliegue.
- Los reintentos/DLQ del correo añaden ruido operativo si el proveedor falla.
- PII: los logs no deben incluir tokens ni correos completos.

## Criterios de aceptación

- Reset: la solicitud responde `ok` sin revelar existencia; la confirmación
  cambia la contraseña; token usado o expirado falla; tests.
- Pedido: un correo por destinatario/evento, idempotente; reintentos y DLQ.
- Tests/E2E: unit e integración con `EMAIL_MODE=log`; el E2E usa SMTP
  **local** (Mailpit en loopback). Nunca un proveedor externo en CI.
- `git grep` no encuentra credenciales SMTP; solo aparecen en `.env*.example`.

## Estado de implementación (2026-11-11)

- Fases 1-5 mergeadas: #536 (`shared/email`), #544 (reset), #545
  (`notifications-service`), #546 (invitaciones) y #547 (E2E con Mailpit).
- `EmailProvider` intercambiable: `log` en dev/test y `smtp` en stage/prod;
  `notifications-service` consume `notifications.todos` con idempotencia en
  `notifications.eventos_procesados`, reintentos y DLQ.
- Reset: token SHA-256 de un solo uso (30 min), sin enumeración de correos y con
  revocación de sesiones al confirmar.
- Invitaciones de personal: envío del enlace de 72 h; un fallo SMTP no invalida
  la invitación.
- E2E real: Mailpit captura el correo, el smoke extrae el enlace y valida
  solicitud, confirmación, un solo uso, login nuevo y revocación de sesiones.
  Mailpit solo vive en `docker-compose.dev.yml` y en loopback.
- Pendiente fuera de alcance: notificaciones push y plantillas de otras
  verticales.
