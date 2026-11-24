# Runbook — activar RLS en stage/producción

La app puede conectarse como `core_engine_app` (NOBYPASSRLS). Para no romper
jobs, cada conexión recibe un claim de servicio por defecto; las peticiones
HTTP lo sobrescriben con el usuario real dentro de su transacción.

## 1. Prerrequisitos

1. `CORE_ENGINE_APP_PASSWORD` definido en el `.env` del entorno.
2. El servicio `db-migrate` aplicó `99z_rls_tenant.sql` (ledger
   `public.schema_migrations`). En una base existente se aplica en el próximo
   arranque; no lo hagas a mano.
3. Backup reciente y restore drill ejecutado.

## 2. Cambio

En el `.env` del entorno (Dockploy → Environment):

```dotenv
DATABASE_URL=postgres://core_engine_app:<password>@<host>:5432/<db>
CORE_ENGINE_APP_PASSWORD=<password>
```

Redesplegar. El arranque de `db-migrate` asegura `LOGIN` y la membership
`authenticated`.

## 3. Verificación

```sql
-- Como core_engine_app, con claims del tenant A:
SELECT set_config('request.jwt.claims',
  '{"rol":"empresario","tenant_id":"<A>"}', false);
SELECT count(*) FROM analytics.hechos_venta;        -- solo A
SELECT count(*) FROM finance.configuracion_fiscal;   -- solo A
SELECT count(*) FROM identity.empresa_region_historial; -- solo A
```

`identity.pack_regiones` y `finance.jurisdicciones` son catálogos de
plataforma: se leen con cualquier claim de tenant, pero solo los administran
`super_admin`/`admin`.

- `GET /api/v2/orders/empresa/resumen` con token de A no debe ver B.
- Un job de outbox debe seguir publicando (claim de servicio).
- `/ready` debe seguir en verde y el checkout de producto funcionar.

## 4. Rollback

Volver a `DATABASE_URL` con el rol owner (`core_engine`) y redesplegar. Las
políticas RLS quedan en la base, pero el owner las omite. No se revierten las
migraciones.

## 5. Límites conocidos

- Las rutas públicas (storefront) no tienen usuario: usan el claim de servicio y
  dependen de sus filtros explícitos por tenant. No son una frontera RLS.
- `identity.usuarios`, `identity.sesiones`, `orders.eventos`,
  `finance.asientos` y outbox quedan fuera de RLS por contrato de auth/jobs.
- Los consumers deben usar `transaccion_servicio()` cuando lean tablas de tenant
  sin usuario; si usan `pool()` directo, heredan el claim de servicio.
