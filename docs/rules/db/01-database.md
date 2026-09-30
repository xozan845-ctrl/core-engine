# 01 — Reglas Doradas de Base de Datos

Aplica a: esquemas por dominio, RLS, transacciones, índices, borrado,
migraciones y paginación en Postgres (local y Supabase).

## Reglas

| ID | Regla |
|---|---|
| R-DB-1 | **Aislamiento por dominio (schema-per-service)**: ningún microservicio puede acceder a las tablas de otro microservicio directamente. Cada servicio debe poseer y ser dueño exclusivo de un `SCHEMA` en Postgres (ej: `orders`, `catalog`, `identity`). Si un servicio requiere datos de otro, debe consumirlos vía API interna o suscribirse a sus eventos para construir una vista materializada local. |
| R-DB-2 | **Uso de Row Level Security (RLS)**: en entornos productivos (Supabase), la seguridad de datos multi-tenant y de acceso por rol debe forzarse siempre a nivel base de datos usando RLS. La identidad (UID, rol) se inyecta por el contexto de la petición mediante `auth.uid()` y `auth.jwt()`. **Importante:** estas funciones solo devuelven datos válidos si el microservicio inyectó previamente el contexto JWT en la conexión de Postgres usando `set_config` (ver procedimiento en `R-SB-3`, `02-supabase.md`). Sin ese paso, `auth.uid()` devuelve `null` y las políticas RLS denegarán silenciosamente todos los accesos. |
| R-DB-3 | **Transaccionalidad estricta**: toda operación que modifique dos o más entidades lógicas o que inserte registros en la tabla `outbox` **debe** envolverse obligatoriamente en una única transacción SQL (`QueryRunner` o similar en el ORM). No existen "transacciones a medias". |
| R-DB-4 | **Índices precisos**: las claves foráneas lógicas, los correos electrónicos o cualquier campo usado frecuentemente para filtrar o en operaciones `JOIN` (si aplican dentro del mismo esquema) deben estar siempre indexados para evitar escaneos de tabla completos secuenciales (`Seq Scan`). |
| R-DB-5 | **No eliminación física (soft delete)**: los datos transaccionales (órdenes, pagos, stock, usuarios) nunca se eliminan con `DELETE`. Se debe usar siempre borrado lógico (ej. `deleted_at`, `status = INACTIVE`) por motivos de auditoría e integridad referencial histórica. **Excepción:** la fase de limpieza (teardown) de las pruebas de integración/E2E sí debe usar eliminación física o `TRUNCATE` para mantener el aislamiento de los tests. |
| R-DB-6 | **Migraciones obligatorias (jerarquía de fuente de verdad)**: NINGÚN cambio de esquema se hará manualmente en producción. La fuente canónica son los scripts SQL: **local (Docker):** `infra/db/init/*.sql` (aplicados automáticamente al iniciar el contenedor); **producción (Supabase):** scripts SQL versionados y aplicados vía `supabase db push` o consola SQL (ver `R-SB-4`, `02-supabase.md`). Los mecanismos de migración del ORM son una alternativa secundaria y deben alinearse con los scripts SQL. |
| R-DB-7 | **Paginación eficiente**: evitar paginación basada en OFFSET para conjuntos de datos grandes, ya que degrada el rendimiento. Preferir paginación basada en cursores (keyset pagination) para historiales, transacciones y catálogos extensos. |
