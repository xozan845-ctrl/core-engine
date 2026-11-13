# ADR-0011: Reescritura de `storage-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación Rust en curso; el TypeScript sigue siendo el runtime activo hasta la conmutación
- **Ámbito**: `storage-service` (HTTP público, admin e interno, PostgreSQL, S3/MinIO, Outbox, métricas)
- **Referencias**: [ADR-0004](./0004-scheduling-service-en-rust.md), [ADR-0006](./0006-notifications-service-en-rust.md), [ADR-0010](./0010-logistics-service-en-rust.md)

## Contexto

`storage-service` era un servicio NestJS (~1.9k líneas) que:

- Gestiona un bucket S3/MinIO por tenant (`t-<uuid>`), cuotas y uso.
- Mantiene metadata de objetos (`storage.objetos`), estados
  `pendiente/activo/eliminado` y URLs prefirmadas de subida/descarga.
- Sirve media pública (`image/*`, `video/mp4`, `video/webm`) con `Range` y
  `HEAD`, excluyendo SVG por seguridad.
- Expone superficie autenticada (`/api/v2/storage`), admin
  (`/api/v2/admin/storage`) e interna (`/internal/storage`).
- Escribe eventos `storage.objeto.subido` y `storage.objeto.eliminado` en
  `storage.outbox`, publicados por un poller RabbitMQ.
- Expone `/health` y `/metrics`.

Se reescribe en Rust siguiendo el patrón ya validado en `scheduling-service`,
`notifications-service`, `field-service`, `logistics-service` y
`market-intelligence-service`.

## Decisión

### 1. Invariantes que no cambian

| Área | Invariante |
|---|---|
| Superficie | mismas rutas/métodos/estados; envelope `{data, meta}`; errores `{codigo, mensaje, detalles?}` |
| Buckets | uno por tenant, prefijo `t-`, cuota por defecto 5 GiB, `usado_bytes` sólo cuenta objetos activos |
| Objetos | estados y columnas actuales; `storage.objetos.objeto` único; baja lógica + borrado físico |
| URLs | prefirmadas con TTL configurable; el host firmado es el endpoint público |
| Límites | whitelist de content-types; máximo genérico 25 MiB y vídeo 100 MiB |
| Media pública | solo `activo`, `publico=true`, imagen raster o MP4/WebM; nunca SVG; `Range` de un tramo y 416 |
| Autorización | tenant obligatorio; admin global con `PLATFORM_TENANT_ID`; autor de la subida o admin para borrar; clave interna en `/internal` |
| Eventos | mismos nombres/payloads; Outbox con poller de 1 s y lote de 50 |
| Fechas | ISO 8601 UTC con milisegundos y `Z` |
| Runtime | puerto 3010; mismas variables de entorno; fail-fast sin DB, RabbitMQ o credenciales MinIO |

### 2. Implementación destino

- Crate `packages/storage-service/rust/` con binario `storage-service`.
- `axum` + `tokio`, `sqlx`, `aws-sdk-s3` (endpoint MinIO, path-style,
  presign), `lapin`, `serde`, `uuid`, `chrono`, `thiserror`.
- Capas: `http/` (router, envelope, rutas), `storage/` (buckets, objetos,
  paginación, rangos), `almacen.rs` (adaptador S3), `outbox.rs`, `rabbit.rs`,
  `auth.rs`, `db.rs`, `config.rs`, `metrics.rs`.
- Estado explícito con `Arc` (equivalente a la DI de NestJS, R-MS-3).

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
2. Integración PostgreSQL con almacén en memoria: cuotas, confirmación,
   listado, descarga, eliminación, outbox y media pública.
3. El OpenAPI versionado no cambia; el binario sirve el mismo documento.
4. Smoke de `staging` en la primera promoción de la imagen.

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Crate Rust completo en `rust/`, CI y pruebas | `cargo` verde y gate de cobertura; TS sigue activo |
| 2 | Conmutación: retirar TS, Dockerfile, compose, scripts, reglas y cobertura | Imagen Rust y CI de servicios sin `storage-service` Node |
| 3 | Promoción `develop → staging` | Árbol idéntico y smoke de staging |
