# storage-service · objetos por tenant

`storage-service` es la **autoridad del almacenamiento de objetos**. Implementa
un bucket por tenant sobre S3/MinIO, URLs prefirmadas de subida/descarga,
cuotas, ciclo de vida de objetos y servicio de **media pública** con soporte de
`Range` para imágenes y vídeo.

- ADR-0024 · un bucket por tenant (histórico: [docs/decisiones.md](../../docs/decisiones.md))
- [ADR-0012 · storage-service en Rust](../../docs/adr/0012-storage-service-en-rust.md)
- Contrato: [`docs/openapi/storage-service/`](../../docs/openapi/storage-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `storage-service` |
| Stack | `axum` 0.8, `sqlx`, `aws-sdk-s3`, `lapin` |
| Puerto | `3010` (`PORT`) |
| Esquema | `storage` |
| Outbox | `storage.outbox` |
| Backend | S3-compatible (MinIO en dev/stage/prod) |
| Capacidad | Ninguna: **servicio siempre activo** |
| Publica | `storage.objeto.subido`, `storage.objeto.eliminado` |
| Consume | — |
| Expuesto al host | No; MinIO publica `9000` en loopback (host firmado) |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | Sí para outbox | — | Broker |
| `OUTBOX_TABLA` | No | `storage.outbox` | Outbox |
| `PORT` | No | `3010` | Puerto |
| `SERVICIO` | No | `storage-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Internos |
| `MINIO_ENDPOINT` | No | `minio` | Host interno del almacén |
| `MINIO_PORT` | No | `9000` | Puerto |
| `MINIO_USE_SSL` | No | `false` | TLS interno |
| `MINIO_ROOT_USER` | **Sí para operar** | — | Access key |
| `MINIO_ROOT_PASSWORD` | **Sí para operar** | — | Secret key |
| `MINIO_REGION` | No | `us-east-1` | Región S3 |
| `MINIO_PUBLIC_ENDPOINT` | No | `MINIO_ENDPOINT` | Host **firmado** en URLs prefirmadas |
| `MINIO_PUBLIC_PORT` | No | `MINIO_PORT` | Puerto firmado |
| `MINIO_PUBLIC_USE_SSL` | No | `MINIO_USE_SSL` | Esquema firmado |
| `STORAGE_BUCKET_PREFIX` | No | `t-` | Prefijo del bucket por tenant |
| `STORAGE_MAX_OBJETO_BYTES` | No | `26214400` (25 MiB) | Máximo general |
| `STORAGE_MAX_VIDEO_BYTES` | No | `104857600` (100 MiB) | Máximo para vídeo |
| `STORAGE_CUOTA_DEFAULT_BYTES` (o `STORAGE_CUOTA_DEFAULT`) | No | `5368709120` (5 GiB) | Cuota inicial |
| `STORAGE_URL_TTL_SEGUNDOS` | No | `900` | Validez de URLs prefirmadas |
| `STORAGE_CONTENT_TYPES_PERMITIDOS` | No | whitelist de 10 MIME | Lista separada por comas |
| `PLATFORM_TENANT_ID` | No | — | Tenant para roles sin tenant |

> Las URLs prefirmadas se firman con `MINIO_PUBLIC_ENDPOINT/PORT/USE_SSL`. En
> producción el público es el dominio HTTPS con `443`/`true`; el endpoint
> interno (`minio:9000`) solo se usa para operar.

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `almacen.rs` | Adaptador S3/MinIO (`AlmacenObjetos`), presign, stats, stream |
| `storage/buckets.rs` | Bucket por tenant, cuota, uso, suspensión y recálculo |
| `storage/objetos.rs` | Solicitud, confirmación, listado, descarga, media pública y baja |
| `storage/range.rs` | Parser `Range` RFC 7233 |
| `storage/pagination.rs` | Paginación de objetos |
| `http/storage.rs` | Rutas públicas, autenticadas, admin e internas |
| `auth.rs` | Tenant, roles y clave interna |
| `outbox.rs` / `rabbit.rs` | Eventos de objeto |

El dominio depende de `AlmacenObjetos`; cambiar de proveedor S3-compatible solo
toca `almacen.rs` (ACL, R-AR-6).

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `storage.buckets` | `tenant_id`, `bucket`, `cuota_bytes`, `usado_bytes`, `estado`, contadores |
| `storage.objetos` | `tenant_id`, `bucket`, `objeto` (clave), nombre original, `content_type`, carpeta, etiquetas, tamaño, `etag`, estado, `publico`, autor, fechas |
| `storage.eventos_procesados` | Idempotencia (reservado) |
| `storage.outbox` | Eventos |

### Bucket

- Nombre: `STORAGE_BUCKET_PREFIX + tenant_id` en minúsculas (p. ej.
  `t-11111111-1111-...`).
- Estado: `activo` o `suspendido`.
- Cuota por defecto 5 GiB; puede ajustarse por admin, nunca por debajo de lo
  usado (`CUOTA_MENOR_AL_USO`).
- `usado_bytes` se mantiene por suma/resta transaccional y puede recalcularse
  de forma idempotente desde la metadata (`POST .../recalcular`).
- Un bucket suspendido rechaza subidas (`ALMACENAMIENTO_SUSPENDIDO`).

### Objeto

Ciclo de vida:

```
pendiente ──confirmar──▶ activo ──eliminar──▶ eliminado
    │                       │
    └── expira/abandona     └── media pública solo si publico=true
```

- `pendiente`: fila creada + URL prefirmada de subida.
- `activo`: el objeto existe en S3, se validó tipo/tamaño y se contabilizó
  cuota.
- `eliminado`: baja lógica + borrado físico en S3; deja de servirse.

---

## 4. Subida en dos pasos

### 1) Solicitar URL prefirmada

`POST /api/v2/storage/subidas`:

```json
{
  "nombre": "logo.png",
  "content_type": "image/png",
  "tamano_bytes": 245760,
  "carpeta": "tiendas/logos",
  "publico": true,
  "descripcion": "Logo principal",
  "etiquetas": { "seccion": "branding" }
}
```

Validaciones:

- `content_type` en la whitelist (`CONTENT_TYPE_NO_PERMITIDO`).
- `tamano_bytes > 0` y ≤ máximo del tipo (`OBJETO_DEMASIADO_GRANDE`).
- Cuota disponible (`CUOTA_EXCEDIDA`).
- Nombre saneado y clave `carpeta/<uuid>/<nombre>`.

Respuesta: `objeto_id`, URL prefirmada `PUT`, cabeceras requeridas y expiración.

### 2) Confirmar

`POST /api/v2/storage/objetos/{id}/confirmar` consulta el objeto en S3:

- Si no existe → 404.
- Si está vacío → lo elimina y devuelve `OBJETO_VACIO`.
- Si el MIME real no está permitido → lo elimina y devuelve
  `CONTENT_TYPE_NO_PERMITIDO`.
- Si excede el máximo → lo elimina y devuelve `OBJETO_DEMASIADO_GRANDE`.
- Si todo es correcto: `pendiente → activo`, actualiza tamaño/etag/MIME real y
  suma cuota en una transacción; emite `storage.objeto.subido`.

Confirmar dos veces es idempotente (`activo` devuelve el objeto).

---

## 5. Descarga y media pública

### Descarga autenticada

`GET /api/v2/storage/objetos/{id}/descarga` devuelve una URL prefirmada `GET`
con TTL (`STORAGE_URL_TTL_SEGUNDOS`, 900 s) para el tenant activo.

### Media pública

`GET|HEAD /api/v2/storage/public/objetos/{id}` **sin autenticación**:

- Solo sirve objetos `activo` y `publico=true`.
- Hace **streaming** desde S3 (los bytes no se cargan en memoria).
- Soporta `Range` (RFC 7233): `bytes=inicio-fin`, `bytes=inicio-`, `bytes=-n`.
  - `206 Partial Content` + `Content-Range` si aplica.
  - `416 Range Not Satisfiable` si es inválido.
- Devuelve `Content-Type`, `Content-Length`, `ETag` y `Accept-Ranges`.
- El gateway lo trata como ruta pública y excluye el rate limit global.

---

## 6. API pública y admin

| Método | Ruta | Rol |
|---|---|---|
| GET | `/api/v2/storage/bucket` | tenant (estado, cuota, uso) |
| POST | `/api/v2/storage/bucket` | admin (asegurar bucket / fijar cuota) |
| POST | `/api/v2/storage/subidas` | tenant (solicitar prefirmada) |
| POST | `/api/v2/storage/objetos/{id}/confirmar` | tenant |
| GET | `/api/v2/storage/objetos` | tenant (paginado, sin eliminados por defecto) |
| GET | `/api/v2/storage/objetos/{id}` | tenant (metadata) |
| DELETE | `/api/v2/storage/objetos/{id}` | tenant (baja lógica + S3) |
| GET | `/api/v2/storage/objetos/{id}/descarga` | tenant (URL prefirmada) |
| GET/HEAD | `/api/v2/storage/public/objetos/{id}` | **público** |
| GET | `/api/v2/admin/storage/tenants` | admin |
| GET/PATCH | `/api/v2/admin/storage/tenants/{tenantId}` | admin |
| POST | `/api/v2/admin/storage/tenants/{tenantId}/recalcular` | admin |

---

## 7. API interna (`x-internal-key`)

| Método | Ruta | Consumidor |
|---|---|---|
| GET | `/internal/storage/tenants/{tenantId}` | identity/stores (estado y cuota) |
| POST | `/internal/storage/tenants/{tenantId}/recalcular` | operación |
| GET | `/internal/storage/objetos/{id}` | identity/stores (metadata para validar medios) |

La metadata interna incluye `tenant_id`, `estado`, `publico`, `content_type` y
tamaño, que es lo que usan identity y stores para aceptar avatares, logos y
fondos.

---

## 8. Eventos

| Evento | Cuándo | Consumidores |
|---|---|---|
| `storage.objeto.subido` | Confirmación de objeto | — (informativo/analítica) |
| `storage.objeto.eliminado` | Baja lógica + borrado S3 | — |

Se escriben en `storage.outbox` en la misma transacción del cambio; el
publicador corre cada segundo.

---

## 9. Seguridad y multi-tenancy

- Cada operación resuelve el tenant del JWT; un objeto de otro tenant responde
  `NO_ENCONTRADO` (no se revela).
- El bucket nunca es compartido entre tenants.
- La media pública solo expone objetos explícitamente `publico=true` y
  `activo`.
- Roles admin solo para listar tenants, ajustar cuota y suspender.
- La clave de objeto incluye UUID, evitando colisiones y enumeración.
- Los MIME se validan dos veces: al solicitar y al confirmar (contra S3).

---

## 10. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas HTTP + outbox.
- Logs JSON con `x-request-id`.
- El MinIO de producción publica solo `9000` en loopback porque ese host es el
  firmado en las URLs; la consola `9001` nunca se publica.
- Apagado ordenado.

### Diagnóstico frecuente

| Síntoma | Causa |
|---|---|
| `CUOTA_EXCEDIDA` | Uso + tamaño > cuota; recalcular o ampliar |
| `ALMACENAMIENTO_SUSPENDIDO` | Bucket suspendido por admin |
| URL prefirmada inválida | Host/puerto/esquema público mal configurado |
| `CONTENT_TYPE_NO_PERMITIDO` | MIME fuera de whitelist o distinto al real |
| `OBJETO_VACIO` | `PUT` no subió bytes |
| Vídeo no reproduce | Rango no soportado o objeto no público |

---

## 11. Pruebas

```bash
cd packages/storage-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/storage_pg.rs`: buckets, cuotas, subida/confirmación, borrado,
  metadata interna y recálculo.
- Tests puros de `range` (RFC 7233) y saneado de nombres.

---

## 12. Referencias

- ADR-0024 · un bucket por tenant (histórico: [docs/decisiones.md](../../docs/decisiones.md))
- [ADR-0012](../../docs/adr/0012-storage-service-en-rust.md)
- [Contrato público](../../docs/openapi/storage-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/storage-service/internal.openapi.json)
- [README de paquetes](../README.md)
