# 04 — Reglas de Contrato de API

Aplica a: la forma de los payloads del API (`/api/v1/**`) y su documentación Swagger.
El frontend consume estos contratos; un cambio inesperado rompe producción sin fallar
los tests unitarios de nadie.

## Shape de respuesta

| ID | Regla |
|---|---|
| R-C-1 | Toda respuesta de éxito sigue el envelope `{ data: T, meta: { timestamp, path, method, statusCode } }`. En listas paginadas la paginación va anidada dentro de `data`: `{ data: { data: T[], total, page, limit, totalPages }, meta }`. El test de contrato valida presencia y tipos en ambos niveles. Los errores **no** usan este envelope (ver R-C-8). |
| R-C-2 | Toda respuesta sigue el patrón de API estándar del proyecto (éxito vs error) y el test de contrato lo valida con un schema (Zod/JSON-Schema o aserturas de shape). |
| R-C-3 | Todo objeto de dominio expuesto en el API tiene un **DTO de respuesta**; el contrato se testea contra el DTO, nunca exponer entidades Prisma crudas (prohibido filtrar `password` u otros campos sensibles). |

## Fechas

| ID | Regla |
|---|---|
| R-C-4 | Todas las fechas en respuestas son **ISO 8601 UTC con Z** (`2026-09-23T14:44:35.000Z`). Test de contrato rechaza offsets no-Z y timestamps no normalizados. |
| R-C-5 | El contrato documenta explícitamente la semántica de los filtros de fecha: `dateFrom`/`dateTo` son **día local** de la zona configurada (`TZ`). |

## Versionado

| ID | Regla |
|---|---|
| R-C-6 | Cambios incompatibles (renombrar/eliminar campo, cambiar tipo) solo en nueva versión `/api/v2`; test de contrato detecta la pérdida de campos. |
| R-C-7 | Swagger se genera de los DTOs (no anotado a mano); el spec se commitea en `backend/docs/openapi.json` (`npm run swagger:export`) y CI lo bloquea con `git diff --exit-code` cuando está desfasado. |

## Errores

| ID | Regla |
|---|---|
| R-C-8 | Todo error responde con shape consistente (`statusCode`, `message`); test de contrato cubre 400/401/403/404/409/500. |
| R-C-9 | Nada de detalles internos en errores 500 (stack traces, SQL) en `NODE_ENV=production`. |

## Automatización

| ID | Regla |
|---|---|
| R-C-10 | Existe un test de contrato que recorre los endpoints críticos y valida response shape contra schema. Se ejecuta en CI como parte de integración. |
| R-C-11 | Si el frontend consume un campo nuevo, el PR que lo agrega incluye: campo en DTO + test de contrato + consumo en frontend (o marcado como futuro). |
