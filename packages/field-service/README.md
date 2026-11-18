# field-service · operación de campo

`field-service` es el **bounded context de operación en terreno**: personal,
clientes de campo, vehículos, rutas y paradas, pedidos de campo, asistencia,
incidencias, tracking GPS, visitas comerciales, cumplimiento de ruta y
sincronización offline.

Es la capacidad `operacion_campo` de las verticales de distribución, flota,
última milla y venta en ruta.

- [ADR-0008 · field-service en Rust](../../docs/adr/0008-field-service-en-rust.md)
- [ADR-0001 · plataforma multi-vertical](../../docs/adr/0001-plataforma-multi-vertical.md)
- Contrato: [`docs/openapi/field-service/`](../../docs/openapi/field-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `field-service` |
| Stack | `axum` 0.8, `sqlx`, `reqwest` |
| Puerto | `3008` (`PORT`) |
| Esquema | `field` |
| Outbox / RabbitMQ | No participa (servicio síncrono) |
| Capacidad | `operacion_campo` |
| Dependencias | `identity` (partes y capacidades) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `PORT` | No | `3008` | Puerto |
| `SERVICIO` | No | `field-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Validación de partes |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Partes/capacidades |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

> **Nota:** aunque el compose define `OUTBOX_TABLA=field.outbox`, el servicio
> actual no publica ni consume eventos. No requiere `RABBITMQ_URL`.

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `field/mod.rs` | Mapas tabla↔JSON, CRUD genérico multi-tenant, defaults, borrado lógico, `sync` |
| `field/validacion.rs` | DTOs con shape `VALIDACION` + `detalles[{campo, restricciones}]` |
| `field/operacion.rs` | Tracking, visitas y sincronización offline |
| `field/rutas.rs` | Rutas, paradas y asignación |
| `field/personal.rs`, `clientes.rs`, `vehiculos.rs`, `pedidos.rs`, `asistencia.rs`, `incidencias.rs` | Esquemas por entidad |
| `field/generico.rs` | Helpers SQL/JSON y validación de partes |
| `db.rs` | Pool, transacciones con claims de RLS, conversión de tipos |
| `http/field.rs` | Handlers por recurso |

El dominio es **genérico por mapa de columnas**: cada tabla declara qué campos
JSON acepta (`MAP_*`), sus defaults y su nombre físico. Esto replica el
comportamiento dinámico del servicio TypeScript sin perder validación.

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `field.personal` | Nombre, cargo, estado, contacto, ruta/vehículo asignado, check-in/out, ubicación |
| `field.clientes` | Cliente de campo (tipo, contacto, dirección, coordenadas) |
| `field.vehiculos` | Tipo, placa, capacidad, estado |
| `field.rutas` | Nombre, fecha, estado, personal asignado (JSONB), vehículo |
| `field.paradas` | Ruta, orden, tipo, cliente, coordenadas, estado |
| `field.pedidos` | Pedido de campo, cliente, líneas, total, estado |
| `field.asistencia` | Check-in/out por personal |
| `field.incidencias` | Tipo, severidad, estado, descripción |
| `field.tracking` | Puntos GPS por personal (`lat`, `lng`, `registrado_en`) |
| `field.visitas` | Telemetría de visitas (inicio/fin, resultado) |
| `field.cumplimiento` | Cumplimiento por ruta y fecha |
| `field.eventos_procesados` / `outbox` | Reservados por infraestructura (sin uso actual) |

### Invariantes

- Todas las tablas llevan `tenant_id` (empresa) y `deleted_at` para baja lógica.
- Todos los listados filtran por tenant y excluyen `deleted_at`.
- `parte_id` se valida contra identity: debe pertenecer a la empresa
  (`PARTE_INVALIDA`).
- Los campos desconocidos se ignoran (equivalente al `whitelist` del pipe).
- Las coordenadas deben ser numéricas y válidas.
- Los defaults replican el contrato: `personal.cargo=conductor`,
  `vehiculos.tipo=camioneta`, `rutas.estado=pendiente`, etc.
- Una ruta puede tener múltiples paradas ordenadas; el orden se conserva.

---

## 4. Recursos y API

Todas las rutas requieren JWT con rol operativo/administrativo y la capacidad
`operacion_campo`.

### Personal

| Método | Ruta |
|---|---|
| GET/POST | `/api/v2/field/personal` |
| GET/PATCH/DELETE | `/api/v2/field/personal/{id}` |
| GET/PATCH | `/api/v2/field/personal/{id}/ubicacion` |

### Clientes y vehículos

`GET/POST /field/clientes`, `GET/PATCH/DELETE /field/clientes/{id}`;
`GET/POST /field/vehiculos`, `GET/PATCH/DELETE /field/vehiculos/{id}`.

### Rutas

| Método | Ruta |
|---|---|
| GET/POST | `/api/v2/field/rutas` |
| GET/PATCH/DELETE | `/api/v2/field/rutas/{id}` |
| POST | `/api/v2/field/rutas/{id}/paradas` |
| PATCH | `/api/v2/field/rutas/{id}/paradas/{paradaId}` |
| POST | `/api/v2/field/rutas/{id}/asignar` |

`GET /field/rutas?personal_id=…` filtra las rutas donde el personal está
asignado (`personal_asignado_ids ?| array[...]`) y adjunta sus paradas.

### Pedidos de campo

`GET/POST /field/pedidos`, `GET/PATCH/DELETE /field/pedidos/{id}`,
`PATCH /field/pedidos/{id}/estado`.

### Asistencia e incidencias

`GET/POST /field/asistencia`;
`GET/POST /field/incidencias`, `GET/PATCH/DELETE /field/incidencias/{id}`.

### Tracking y visitas

| Método | Ruta | Notas |
|---|---|---|
| GET | `/field/tracking?personal_id&desde&hasta` | Telemetría GPS |
| POST | `/field/tracking` | Acepta lote (`registros`) |
| GET/POST | `/field/visitas` | Inicio/fin y resultado |

### Cumplimiento

- `GET /field/cumplimiento/{rutaId}/{fecha}`: cumplimiento de una ruta en una
  fecha.
- `POST /field/cumplimiento`: guarda el cumplimiento.

### Sincronización offline

`POST /api/v2/field/sync` con:

```json
{
  "operaciones": [
    { "tipo": "visitas", "payload": { "id": "uuid-local", "…": "…" } },
    { "tipo": "tracking", "payload": { "id": "uuid-local", "…": "…" } }
  ]
}
```

- Cada operación se procesa de forma independiente; el fallo de una **no**
  aborta las demás.
- El `id` del payload actúa como clave de idempotencia/upsert: reintentar la
  sincronización no duplica.
- La respuesta es un arreglo
  `[{tipo, id, ok, error?}]` en el mismo orden.
- Tipos soportados: los que tienen tabla de sync (`personal`, `clientes`,
  `vehiculos`, `rutas`, `paradas`, `pedidos`, `asistencia`, `incidencias`,
  `tracking`, `visitas`). Un tipo desconocido devuelve `ok:false`.

---

## 5. Seguridad y multi-tenancy

- RLS con `request.jwt.claims`; además filtro explícito por `tenant_id`.
- El rol operativo puede registrar su propia operación; administración puede
  gestionar todo el tenant.
- `parte_id` se valida contra identity para no enlazar clientes de otro tenant.
- Baja lógica: nunca se borran filas con historia.
- Los errores de validación usan el formato común y no exponen SQL.

---

## 6. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Logs JSON con `x-request-id`; `sync` registra tenant y número de operaciones.
- Sin workers ni consumidores; el proceso es stateless respecto a RabbitMQ.
- Apagado ordenado.

---

## 7. Pruebas

```bash
cd packages/field-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/field_pg.rs`: CRUD, filtros por tenant, rutas/paradas, tracking,
  visitas, cumplimiento y sync idempotente.
- `tests/http_field.rs`: contrato HTTP, validación, roles y errores.
- Casos puros de validación de DTOs y defaults.

---

## 8. Referencias

- [ADR-0008](../../docs/adr/0008-field-service-en-rust.md)
- [ADR-0001](../../docs/adr/0001-plataforma-multi-vertical.md)
- [Contrato público](../../docs/openapi/field-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/field-service/internal.openapi.json)
- [README de paquetes](../README.md)
