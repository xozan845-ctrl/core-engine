# storage-service

Almacenamiento de objetos **multi-tenant** del MVP: **un bucket de MinIO por tenant**
(ADR-24), con subidas y descargas mediante **URLs prefirmadas** y cuotas por organizacion.
La identidad y el tenant vienen del `identity-service` a traves del gateway (JWT HS256 →
cabeceras `x-user-*` y `x-tenant`); el servicio nunca valida el token por su cuenta (R-GW-3).

## Como funciona

```text
cliente ──1. POST /api/v1/storage/subidas ─────▶ storage-service
        ◀── URL prefirmada (PUT) + objeto_id ───         │ valida tipo/tamano/cuota
                                                         │ registra objeto 'pendiente'
        ──2. PUT <url prefirmada> ──────────────▶ MinIO (bucket t-<tenantId>)
        ──3. POST /objetos/:id/confirmar ───────▶ storage-service
                                                         │ statObject + cuota real
                                                         │ objeto 'activo' + evento outbox
        ──4. GET /objetos/:id/descarga ─────────▶ storage-service
        ◀── URL prefirmada (GET, TTL corto) ────         │
```

- **Bucket por tenant**: `t-<tenantId>` (S3 exige minusculas). Se crea de forma perezosa la
  primera vez que el tenant usa el almacenamiento.
- **Cuotas**: `storage.buckets` guarda cuota y uso por tenant; `storage.objetos` la metadata
  de cada archivo. Los objetos `pendiente` **no** consumen cuota hasta confirmarse.
- **Límites**: whitelist de content-types y tamano máximo por objeto (`STORAGE_*`).
- **Eventos** (outbox, R-AR-3): `storage.objeto.subido` y `storage.objeto.eliminado`.

## Endpoints (via gateway)

| Método | Ruta | Rol | Descripción |
|---|---|---|---|
| POST | `/api/v1/storage/subidas` | autenticado | Solicita URL prefirmada de subida (valida tipo/tamano/cuota) |
| POST | `/api/v1/storage/objetos/:id/confirmar` | autenticado | Verifica en MinIO y activa el objeto (contabiliza cuota) |
| GET | `/api/v1/storage/objetos` | autenticado | Lista paginada del tenant (sin eliminados por defecto) |
| GET | `/api/v1/storage/objetos/:id` | autenticado | Metadata del objeto |
| GET | `/api/v1/storage/objetos/:id/descarga` | autenticado | URL prefirmada de descarga (TTL corto) |
| DELETE | `/api/v1/storage/objetos/:id` | autor o admin | Baja logica + borrado en MinIO + libera cuota |
| GET | `/api/v1/storage/bucket` | autenticado | Estado del tenant (bucket, cuota, uso, objetos) |
| POST | `/api/v1/storage/bucket` | admin | Asegura el bucket y fija su cuota |
| GET | `/api/v1/admin/storage/tenants` | admin | Inventario multi-tenant (cuota y uso) |
| GET/PATCH | `/api/v1/admin/storage/tenants/:tenantId` | admin | Estado / ajustar cuota y suspender |
| POST | `/api/v1/admin/storage/tenants/:tenantId/recalcular` | admin | Recalcula el uso desde la metadata |
| GET/POST | `/internal/storage/...` | clave interna | Estado y recalculo (servicio→servicio) |

## Variables de entorno

| Variable | Por defecto | Descripción |
|---|---|---|
| `MINIO_ROOT_USER` / `MINIO_ROOT_PASSWORD` | — (**obligatorias**) | Credencial del servicio en MinIO |
| `MINIO_ENDPOINT` / `MINIO_PORT` / `MINIO_USE_SSL` | `minio` / `9000` / `false` | Endpoint interno (operaciones) |
| `MINIO_PUBLIC_ENDPOINT` / `MINIO_PUBLIC_PORT` / `MINIO_PUBLIC_USE_SSL` | = internos | Host **firmado** en las URLs prefirmadas (el que resuelve el cliente) |
| `STORAGE_BUCKET_PREFIX` | `t-` | Prefijo de los buckets por tenant |
| `STORAGE_MAX_OBJETO_BYTES` | `26214400` (25 MiB) | Tamano máximo por objeto |
| `STORAGE_CUOTA_DEFAULT_BYTES` | `5368709120` (5 GiB) | Cuota inicial de cada tenant |
| `STORAGE_URL_TTL_SEGUNDOS` | `900` | Validez de las URLs prefirmadas |
| `STORAGE_CONTENT_TYPES_PERMITIDOS` | whitelist de imágenes/PDF/CSV/JSON | Tipos aceptados |

> **El host firmado importa**: si `MINIO_PUBLIC_ENDPOINT` no resuelve desde el cliente, las
> URLs prefirmadas no sirven. En stage/producción es la IP/dominio con el que se alcanza el
> 9000 de MinIO (o el dominio HTTPS que le asigne Dockploy, con `MINIO_PUBLIC_USE_SSL=true`).

## Desarrollo

```bash
docker compose -f docker-compose.dev.yml up -d --build minio storage-service
curl -s localhost:9000/minio/health/live   # MinIO arriba
npm run test -w @core/storage-service -- --coverage
```

El flujo completo (subir, confirmar, descargar) se prueba con el smoke local o con
`curl` contra el gateway (`:8080`), usando un token que lleve `tenant_id`.
