# scheduling-service · agenda multi-vertical

`scheduling-service` es la **autoridad de la agenda**: recursos reservables
(profesionales, salas, sillones, vehículos), disponibilidad semanal por zona
horaria, bloqueos, citas y recordatorios. Sirve a verticales de servicios
(belleza, dental, taller, consultoría) sin que exista un servicio por rubro.

- [ADR-0004 · scheduling-service en Rust](../../docs/adr/0004-scheduling-service-en-rust.md)
- [ADR-0001 · plataforma multi-vertical](../../docs/adr/0001-plataforma-multi-vertical.md)
- Contrato: [`docs/openapi/scheduling-service/`](../../docs/openapi/scheduling-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `scheduling-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest`, `chrono-tz` |
| Puerto | `3011` (`PORT`) |
| Esquema | `scheduling` |
| Outbox | `scheduling.outbox` |
| Capacidad | `agenda_recursos` |
| Consume | `order.created` (cola `scheduling.orders`) |
| Publica | `appointment.created`, `appointment.cancelled`, `appointment.reminder.due` |
| Dependencias | `identity` (partes y capacidades) |
| Worker | recordatorios cada 60 s |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor + outbox |
| `OUTBOX_TABLA` | No | `outbox` (en compose se define `scheduling.outbox`) | Outbox |
| `PORT` | No | `3011` | Puerto |
| `SERVICIO` | No | `scheduling-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Internos + identity |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Partes/capacidades |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `scheduling/mod.rs` | Tipos de recurso, disponibilidad, bloqueos y validaciones |
| `scheduling/recursos.rs` | CRUD de recursos, disponibilidad y bloqueos |
| `scheduling/citas.rs` | Crear/listar/cancelar citas con solapamiento |
| `scheduling/recordatorios.rs` | Programación y despacho de recordatorios; reserva desde orden |
| `scheduling/helpers.rs` | Validación de disponibilidad y creación de recordatorios |
| `scheduling/puerto.rs` | `AgendaPort` para tests/consumidores |
| `tareas.rs` | Consumidor, outbox y worker |
| `http/agenda.rs` | Handlers públicos e internos |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `scheduling.recursos` | `tipo`, `nombre`, `capacidad`, `atributos`, `estado` |
| `scheduling.disponibilidad` | `dia_semana` (0–6), `inicia_hora`, `termina_hora`, `zona_horaria`, vigencia |
| `scheduling.bloqueos` | `periodo` (`tstzrange`), motivo, actor |
| `scheduling.citas` | `recurso_id`, `parte_id`, `order_id`, `item_id`, `inicia_en`, `termina_en`, `estado`, notas |
| `scheduling.recordatorios` | `tipo` (`24h`, `2h`), `programado_en`, `estado` |
| `scheduling.eventos_procesados` | Idempotencia |
| `scheduling.outbox` | Eventos |

### Invariantes

- Tipos de recurso: `profesional`, `sala`, `sillon`, `vehiculo`, `otro`.
- `inicia_en < termina_en`; sin fechas inválidas (`PERIODO_INVALIDO`).
- La cita debe caer **dentro** de un intervalo de disponibilidad del recurso,
  en el día de la semana y zona horaria de ese intervalo, y dentro de su
  vigencia.
- No puede solaparse con un **bloqueo** (`tstzrange && tstzrange`).
- No puede solaparse con otra cita en estado activo: `reservada`,
  `confirmada`, `en_atencion`.
- El recurso debe estar `activo` y pertenecer a la empresa.
- La parte debe pertenecer al tenant (`PARTE_INVALIDA` si no).
- Una reserva de orden exige `recurso_id`, fechas y cantidad 1 (validado en
  orders).

---

## 4. Agenda y disponibilidad

### Recurso

`POST /api/v2/scheduling/recursos` con `tipo`, `nombre`, `capacidad`,
`atributos` JSON. `GET /api/v2/scheduling/recursos` lista por estado y nombre.

### Disponibilidad semanal

`POST /api/v2/scheduling/disponibilidad` define un intervalo:

```json
{
  "recurso_id": "uuid",
  "dia_semana": 1,
  "inicia_hora": "09:00",
  "termina_hora": "18:00",
  "zona_horaria": "America/Managua",
  "vigente_desde": "2026-01-01",
  "vigente_hasta": null
}
```

- `dia_semana` usa `EXTRACT(DOW)`: 0 = domingo, 6 = sábado.
- Si no se indica zona, se usa la del intervalo/negocio; los cálculos se hacen
  en la zona del registro, no en UTC.
- `GET /recursos/{recursoId}/disponibilidad` lista la configuración.

### Bloqueos

`POST /api/v2/scheduling/bloqueos` con `recurso_id`, `inicia_en`, `termina_en`,
`motivo`. Se guardan como rango `[inclusive, exclusive)` y excluyen cualquier
cita que se solape en el tiempo.

---

## 5. Citas

| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/v2/scheduling/citas?desde&hasta` | Listar citas de la empresa |
| POST | `/api/v2/scheduling/citas` | Crear cita con validación de disponibilidad |
| PATCH | `/api/v2/scheduling/citas/{citaId}/cancelar` | Cancelar cita activa |

Crear cita ejecuta, en una transacción:

1. `FOR UPDATE` del recurso activo de la empresa.
2. Validación de disponibilidad (día, hora, zona, vigencia).
3. Verificación de bloqueos solapados.
4. Verificación de citas activas solapadas.
5. Inserción de la cita.
6. Creación de recordatorios **24 h** y **2 h** antes (solo si aún son
   futuros).
7. Outbox `appointment.created`.

Cancelar solo afecta a citas activas; emite `appointment.cancelled`.

### Reserva desde una orden

El consumidor de `order.created` busca una línea de tipo `servicio`/`alquiler`
con `fulfillment=onsite` y crea la cita vinculada a `order_id` de forma
idempotente (si ya existe una cita con ese `order_id`, la devuelve). Los datos
de recurso/fechas vienen en `fulfillment_config` de la orden.

---

## 6. Recordatorios

- Cada cita activa genera dos recordatorios: **24 h** y **2 h** antes.
- El worker corre cada **60 segundos**, reclama hasta 100 pendientes con
  `FOR UPDATE SKIP LOCKED`, los marca `enviado` y emite
  `appointment.reminder.due`.
- `GET /api/v2/scheduling/recordatorios` lista los últimos 500.
- La entrega efectiva (correo/push) corresponde a `notifications-service` o al
  consumidor del evento; scheduling no envía mensajes.

---

## 7. API interna (`x-internal-key`)

| Método | Ruta | Uso |
|---|---|---|
| POST | `/internal/scheduling/disponibilidad` | Verificar disponibilidad de un recurso en un rango (orders) |

Devuelve éxito si el recurso está libre; error `RECURSO_NO_DISPONIBLE` (409)
si no.

---

## 8. Eventos

### Consume (`scheduling.orders`)

| Evento | Efecto |
|---|---|
| `order.created` | Si `tipo_orden=reserva` y hay línea onsite, materializa la cita |

Idempotencia por `event_id` en `scheduling.eventos_procesados` y, además, por
`order_id` en `citas`.

### Publica

| Evento | Cuándo |
|---|---|
| `appointment.created` | Cita creada (manual o desde orden) |
| `appointment.cancelled` | Cita cancelada |
| `appointment.reminder.due` | Recordatorio vencido y despachado |

---

## 9. Seguridad y multi-tenancy

- Filtro por `empresa_id` en todas las tablas.
- La capacidad `agenda_recursos` se valida contra identity (excepto
  `super_admin`/`admin`).
- Permisos de agenda (`agenda.*`) además del rol global.
- La parte se valida contra identity: no se agenda a un cliente de otro tenant.
- Los internos exigen `x-internal-key`.

---

## 10. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Logs JSON con `x-request-id`; worker con
  `scheduling-worker-<timestamp>`.
- Outbox cada segundo; supervisor RabbitMQ y reintento cada 10 s.
- Apagado ordenado: aborta worker y outbox.

### Diagnóstico frecuente

| Síntoma | Causa |
|---|---|
| `RECURSO_NO_DISPONIBLE` | Fuera de horario, bloqueo o cita solapada |
| `CITA_CONFIGURACION_REQUERIDA` | Reserva sin recurso/fechas |
| `PARTE_INVALIDA` | Cliente de otro tenant |
| Sin cita tras reservar | Capacidad inactiva o `order.created` no consumido |

---

## 11. Pruebas

```bash
cd packages/scheduling-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/agenda_pg.rs`: recursos, disponibilidad por zona, bloqueos,
  solapamientos, citas, recordatorios y reserva desde orden.
- `scheduling/tests.rs`: validaciones puras y `AgendaPort` con dobles.

---

## 12. Referencias

- [ADR-0004](../../docs/adr/0004-scheduling-service-en-rust.md)
- [ADR-0001](../../docs/adr/0001-plataforma-multi-vertical.md)
- [Contrato público](../../docs/openapi/scheduling-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/scheduling-service/internal.openapi.json)
- [README de paquetes](../README.md)
