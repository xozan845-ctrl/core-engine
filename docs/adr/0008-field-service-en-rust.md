# ADR-0008: Reescritura de `field-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación completa; smoke de `staging` pendiente
  en la primera promoción de la imagen
- **Ámbito**: `field-service` (HTTP público, PostgreSQL y RLS)
- **Referencias**: [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0004](./0004-scheduling-service-en-rust.md),
  [ADR-0006](./0006-notifications-service-en-rust.md)

## Contexto

`field-service` es el servicio NestJS de logística de campo (app-test): ~1.700
líneas de TypeScript no-spec y 45 endpoints bajo `/api/v2/field/**`.

- CRUD multi-tenant sobre `field.personal`, `clientes`, `vehiculos`, `rutas`,
  `paradas`, `pedidos`, `asistencia`, `incidencias`, `tracking`, `visitas` y
  `cumplimiento`, con borrado lógico (`deleted_at`) y `tenant_id` en todas las
  consultas.
- Envelope `{data, meta}` en las respuestas públicas y errores
  `{codigo, mensaje, detalles?}` sin `meta`.
- Autorización por roles de campo, capacidad `operacion_campo` en todas las
  rutas y `logistica_rutas` adicional en el bloque de rutas/paradas. Las
  capacidades se consultan a la API interna de identity.
- Validación con `class-validator` y el shape `VALIDACION`
  (`detalles[{campo, restricciones}]`).
- `parte_id` opcional validado contra `identity` (servicio→servicio) antes de
  persistir clientes, paradas, pedidos y visitas.
- `/health`, `/metrics` Prometheus y `/docs` con el OpenAPI versionado.
- No consume RabbitMQ, no publica outbox y no tiene worker.

El servicio está **sin migración en curso**: no existe rama, worktree ni
sesión de trabajo sobre `field-service` distinta de esta. Las migraciones
activas son `notifications-service` (ADR-0006, ya integrada) y
`commissions-service` (ADR-0007), y `logistics-service` se trabaja en un
worktree aparte. Se elige `field-service` por su aislamiento (solo HTTP + PG),
su contrato dorado y su tamaño acotado.

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismas 45 rutas, métodos y códigos (201 en POST, 200 en PATCH/DELETE, 400/401/403/404/500); envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores sin `meta` |
| Datos | mismas tablas/columnas, `SELECT *` con los mismos nombres snake_case, `jsonb` como objeto/array y fechas ISO 8601 con milisegundos y `Z`; `date` a medianoche local (`America/Managua`) |
| Multi-tenancy | `tenant_id` obligatorio en todas las consultas y claims `sub`/`rol`/`tenant_id` en `request.jwt.claims` dentro de la transacción; RLS intacta |
| Defaults | `personal.cargo='conductor'`, `estado='activo'`; `clientes.tipo_cliente='particular'`; `vehiculos.tipo='camioneta'`, `estado='disponible'`; `rutas.estado='pendiente'`, `personalIds=[]`; `paradas.tipo='cliente'`, `completada=false`; `pedidos.estado='pendiente'`; `incidencias.estado='abierta'`; `visitas.resultado='no_visitado'` |
| Escrituras | INSERT/UPDATE dinámicos por mapa de columnas; `synced=true`; `created_at`/`updated_at`; `updated_at=NOW()`; upsert de sync `ON CONFLICT (id) DO UPDATE` con `deleted_at=NULL`; borrado lógico sin comprobar existencia |
| Validación | mismas reglas (`isString`, `isEmail`, `isIn`, `isNumber`, `isInt`, `min`, `isBoolean`, `isArray`, `isObject`, `isDateString`, `isUuid`) y mismo shape de error |
| Autorización | mismos roles por ruta; `super_admin`/`admin` omiten capacidades; el resto exige tenant y `operacion_campo` (+`logistica_rutas` en rutas); si identity falla → `503 SERVICIO_NO_DISPONIBLE`; capacidad faltante → `403 CAPACIDAD_NO_ACTIVA` con `{empresa_id, capacidades}` |
| Partes | `parteId` no string → `PARTE_INVALIDA`; identity 404 → `PARTE_INVALIDA`; identity caída → `503`; el campo nulo en un PATCH opcional se valida igual que el TS |
| Filtros | `personal_asignado_ids ?| array[...]`, `DATE(... AT TIME ZONE 'America/Managua')`, rangos de `registrado_en`, `ORDER BY created_at|registrado_en DESC LIMIT 2000` |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` con `core_engine_http_*` y etiquetas `{servicio, metodo, ruta, estado}`; logs JSON con `request_id`, `servicio` y nivel |
| Runtime | puerto 3008 (`PORT`), fail-fast si falta `DATABASE_URL`, cierre ordenado ante `SIGINT`/`SIGTERM` |

### 2. Implementación destino

- Crate en la raíz de `packages/field-service/`, binario `field-service`; el
  TypeScript y el workspace npm se retiran en la conmutación.
- `axum` + `tokio`, `sqlx` (Postgres runtime Tokio), `reqwest` para identity,
  `serde`/`serde_json`, `uuid`, `chrono`, `thiserror` y `tracing`-equivalente
  propio en `logs.rs`.
- Capas: `http/` (router, envelope, correlación, métricas), `auth.rs`
  (contexto, roles, capacidades), `clients.rs` (partes), `db.rs` (pool, RLS,
  conversión de filas a JSON), `field/` (dominio y validación), `state.rs`.
- Inyección explícita por `AppState` con `Arc` (enmienda R-MS-3), igual que
  ADR-0004/0006.

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` y
   `cargo test` verdes.
2. Suite PostgreSQL (`tests/field_pg.rs`): CRUD, borrado lógico, rutas con
   paradas ordenadas, filtros, asistencia con timezone, tracking, incidencias,
   visitas, upsert de cumplimiento, sync offline y validación de partes.
3. Suite HTTP (`tests/http_field.rs`): router completo con envelope, guards,
   validación, 404/401/403 y capacidad de rutas.
4. Los OpenAPI versionados no cambian; el binario sirve el público en
   `/docs-json` y una prueba falla si el `x-servicio` no coincide.
5. Cobertura Rust en el job `rust` de CI (`cargo-llvm-cov`): baseline
   **78,52 % líneas / 72,38 % funciones**, gate 77/71 (redondeo a la baja con
   margen de 1 punto).
6. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR y el índice | Decisión registrada |
| 2 | Crate Rust: configuración, HTTP, envelope, errores, métricas, auth y clientes | `cargo test` verde |
| 3 | Dominio `field` y validación DTO | Paridad de contrato y filas de BD |
| 4 | Pruebas PostgreSQL y HTTP, Dockerfile e imagen | Integración y cobertura verdes |
| 5 | Conmutación: retirar TypeScript, actualizar scripts, compose, CI y docs | `npm run build`, `npm run test:integration` y smoke verdes |

## Consecuencias

- El servicio deja de depender de Node/NestJS y del workspace npm; se reduce la
  imagen y el consumo de memoria.
- `field-service` sale del runner Vitest y del baseline de cobertura Node; su
  gate pasa al job `rust`.
- No hay cambios de contrato, esquema ni variables de entorno obligatorias.
