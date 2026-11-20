# notifications-service · correo transaccional del pedido

`notifications-service` consume los eventos del ciclo del pedido y envía
**correo transaccional** al comprador y, cuando aplica, al vendedor. No expone
API pública de negocio: su superficie HTTP es solo operativa (`/health`,
`/metrics`, `/docs`).

- [ADR-0003 · notificaciones por correo](../../docs/adr/0003-notificaciones-correo.md)
- [ADR-0006 · notifications-service en Rust](../../docs/adr/0006-notifications-service-en-rust.md)
- Contrato: [`docs/openapi/notifications-service/`](../../docs/openapi/notifications-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `notifications-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `lettre`, `reqwest` |
| Puerto | `3012` (`PORT`) |
| Esquema | `notifications` |
| Outbox | Ninguno (solo consume) |
| Capacidad | Ninguna: **siempre activo** |
| Consume | 7 eventos del pedido (cola `notifications.todos`) |
| Publica | — |
| Dependencias | `orders` (detalle de la orden), `identity` (comprador/vendedor) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | Idempotencia (`notifications.eventos_procesados`) |
| `RABBITMQ_URL` | **Sí** | — | Consumidor |
| `PORT` | No | `3012` | Puerto |
| `SERVICIO` | No | `notifications-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Clientes internos |
| `ORDERS_SERVICE_URL` | No | `http://orders-service:3004` | Orden |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Usuarios |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |
| `EMAIL_MODE` | No | `log` | `log` o `smtp` |
| `EMAIL_FROM` | Sí con `smtp` | — | Remitente |
| `SMTP_HOST` | Sí con `smtp` | — | Servidor |
| `SMTP_PORT` | No | `587` | 465 activa TLS directo |
| `SMTP_USER` / `SMTP_PASSWORD` | Juntos | — | Autenticación |

Con `EMAIL_MODE=smtp` faltan host/remitente o el par usuario/contraseña está
incompleto → **el proceso no arranca**.

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `consumidor.rs` | Consumo de `notifications.todos`, idempotencia por `event_id` |
| `notificaciones.rs` | Orquestación: valida orden, resuelve destinatarios, construye y envía |
| `plantillas.rs` | Funciones puras que generan asunto/texto/HTML por evento |
| `clientes.rs` | Clientes internos a orders e identity |
| `email.rs` | `EmailProvider`: `log` o SMTP (`lettre`) |
| `tipos.rs` | DTOs externos mínimos y destinatarios |
| `db.rs` | `eventos_procesados` |
| `http/mod.rs` | Solo infraestructura + documentación |

**Sin envelope, sin capacidad, sin eventos propios.** El servicio es un
consumidor puro.

---

## 3. Eventos consumidos

Cola única `notifications.todos` con 7 routing keys:

| Evento | Destinatario | Contenido |
|---|---|---|
| `order.created` | — | **No genera correo**: se consume para marcar idempotencia; el primer aviso útil es el pago |
| `payment.procesado` | Comprador **y** vendedor | Pago confirmado; el vendedor debe preparar |
| `shipment.started` | Comprador | Guía de despacho (si viene) y total |
| `order.status.updated` | Comprador; vendedor si `cancelada`/`devuelta` | Estado legible y motivo |
| `order.completado` | Comprador | Pedido entregado |
| `devolucion.solicitada` | Comprador | Devolución registrada y motivo |
| `stock.fallido` | Comprador | No se pudo reservar; pedido se cancelará |

### Resolución de destinatarios

1. El evento debe traer `data.order_id` UUID; si no, se registra
   `evento_sin_orden` y se descarta sin reintento.
2. Se consulta la orden a `orders` (`GET /internal/orders/{id}`).
3. Se resuelven comprador y vendedor por `identity`
   (`GET /internal/usuarios/{id}`).
4. Si un usuario no existe (404), se omite ese mensaje; no se reintenta.
5. Si orders/identity devuelven error de red o 5xx, se propaga el fallo para
   que RabbitMQ reintente y, agotados los intentos, derive a DLQ.

### Idempotencia

- `notifications.eventos_procesados(event_id)`.
- El evento se marca **después** de enviar: si el envío falla, no se marca y el
  reintento vuelve a intentar.
- Si ya está marcado, se descarta silenciosamente.

---

## 4. Plantillas y correo

- Asunto y cuerpo se generan en `plantillas.rs` como funciones **puras**.
- Montos: `C$`/`US$` con dos decimales, calculados desde `total_cents`.
- HTML escapado (`&`, `<`, `>`, `"`, `'`); se envía `text/plain` + `text/html`.
- El número de pedido se muestra corto (8 caracteres).
- El estado se traduce a texto legible (`pagada`, `enviada`, …).

### Modos de envío

| `EMAIL_MODE` | Comportamiento |
|---|---|
| `log` | No envía. Escribe una traza con destinatario **enmascarado** (`a***@dominio`) y asunto; nunca cuerpo, token ni correo completo |
| `smtp` | Envía por `lettre`; puerto 465 usa TLS directo, el resto STARTTLS |

En dev/E2E se usa Mailpit (`1025`/`8025`) para capturar los correos.

---

## 5. Superficie HTTP

| Ruta | Contenido |
|---|---|
| `GET /health` | Liveness |
| `GET /metrics` | Prometheus |
| `GET /docs` | Swagger UI |
| `GET /docs-json` | Contrato público |
| `GET /docs-internal-json` | Contrato interno |
| `GET /api/v2/admin/notifications/plantillas` | Plantillas efectivas |
| `PUT /api/v2/admin/notifications/plantillas/{evento}/{canal}` | Crear/editar plantilla |

Las plantillas viven en `notifications.plantillas` (globales o por empresa).
Canales opcionales: `NOTIFICACIONES_CANALES=email,whatsapp,sms`, con
`WHATSAPP_TOKEN`/`WHATSAPP_PHONE_ID` y `SMS_URL`/`SMS_KEY`. Variables de
plantilla: `{{pedido}}`, `{{total}}`, `{{estado}}`, `{{motivo}}`, `{{guia}}`,
`{{cliente}}`, `{{vendedor}}`.

---

## 6. Seguridad y privacidad

- No lee esquemas ajenos: solo APIs internas con `x-internal-key`.
- Los logs enmascaran el correo y omiten el cuerpo.
- No se persiste el contenido del correo ni datos personales más allá de la
  idempotencia del evento.
- Los enlaces y tokens de negocio (reset, invitación) no pasan por este
  servicio; los envía `identity` con su propio proveedor.
- El correo saliente depende de SMTP; en producción se configura por entorno
  (nunca en el repo).

---

## 7. Observabilidad y operación

- `/health`, `/metrics`, `/docs`.
- Logs JSON: `notifications.pedidos`, `notifications.consumer`,
  `notifications.todos`.
- Supervisor RabbitMQ, backoff exponencial, `REINTENTOS_MAXIMOS` y DLQ.
- Métricas HTTP (solo diagnóstico) y contadores de envío.
- Apagado ordenado.

### Diagnóstico frecuente

| Síntoma | Causa |
|---|---|
| No llegan correos | `EMAIL_MODE=log` o cola no consumida |
| Reintentos infinitos | orders/identity caídos (5xx/red) |
| Correo duplicado | Evento reprocesado sin `event_id` estable |
| `evento_sin_orden` | Evento sin `data.order_id` UUID |
| Arranque falla | Config SMTP incompleta |

---

## 8. Pruebas

```bash
cd packages/notifications-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/notificaciones_pg.rs`: idempotencia, marcado tras envío y reintento
  ante fallo.
- Tests de plantillas: contenido por evento, escapado HTML y montos.
- Tests de configuración: validación de `EMAIL_MODE`/SMTP.

---

## 9. Referencias

- [ADR-0003](../../docs/adr/0003-notificaciones-correo.md)
- [ADR-0006](../../docs/adr/0006-notifications-service-en-rust.md)
- [Contrato público](../../docs/openapi/notifications-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/notifications-service/internal.openapi.json)
- [README de paquetes](../README.md)
