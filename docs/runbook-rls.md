# Runbook — RLS obligatorio en stage/producción

Desde ADR-0054, `apply.sh` aplica **siempre** `99_rls.sql` y
`99z_rls_hardening.sql`. Ya no existe `APPLY_RLS=false`. La app se conecta como
`core_engine_app` (`NOSUPERUSER NOBYPASSRLS`) y cada conexión recibe un claim de
servicio; las peticiones HTTP lo sobrescriben con el usuario real.

## 1. Prerrequisitos

1. `CORE_ENGINE_APP_PASSWORD` definido en el `.env` del entorno.
2. Backup reciente y restore drill ejecutado.
3. `DATABASE_URL` de los servicios apuntando a `core_engine_app`, no al owner.
4. Espacio para el `ALTER TABLE ... FORCE ROW LEVEL SECURITY` (bloqueo breve).

## 2. Aplicación

El servicio `db-migrate` aplica en orden:

1. migraciones `infra/db/init/*.sql`;
2. `supabase/99_rls.sql`;
3. `supabase/99z_rls_hardening.sql`.

Si una base existente no tiene ledger, el runner registra el baseline y luego
aplica RLS. No ejecutes los archivos a mano salvo emergencia documentada.

## 3. Verificación

Como owner, comprobar cobertura:

```sql
SELECT count(*) AS tablas_sin_rls
  FROM pg_tables
 WHERE schemaname NOT IN ('pg_catalog','information_schema','public','auth')
   AND NOT rowsecurity;

SELECT count(*) AS sin_force
  FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
 WHERE n.nspname NOT IN ('pg_catalog','information_schema','public','auth')
   AND c.relkind = 'r' AND c.relrowsecurity AND NOT c.relforcerowsecurity;

SELECT count(*) AS policies_true
  FROM pg_policies
 WHERE schemaname NOT IN ('pg_catalog','information_schema','public','auth')
   AND (qual = 'true' OR with_check = 'true');
```

Esperado: `0`, `0`, `0`.

Como `core_engine_app` con claims del tenant A:

```sql
SELECT set_config('request.jwt.claims',
  '{"rol":"empresario","tenant_id":"<A>"}', false);
SELECT count(*) FROM identity.usuarios;        -- solo A
SELECT count(*) FROM orders.pagos;             -- solo A
SELECT count(*) FROM finance.comprobantes;     -- solo A
SELECT count(*) FROM storage.objetos;          -- solo A
```

Prueba automática A/B:

```bash
RLS_TEST_DATABASE_URL=postgres://owner:...@host/db \
RLS_TEST_APP_URL=postgres://core_engine_app:...@host/db \
npm run test:rls
```

En CI, el job `rls-integration` levanta PostgreSQL/PostGIS, aplica todas las
migraciones + RLS y ejecuta esa prueba. `npm run db:rls` sigue siendo un gate
estático sobre los archivos SQL; la fuente de verdad en runtime es
`npm run test:rls` y las consultas de cobertura de esta sección.

Comprobaciones funcionales:

- `GET /api/v2/orders/empresa/resumen` con token de A no debe ver B.
- Checkout, pedidos, finanzas y storage del tenant A funcionan.
- Un job de outbox sigue publicando (claim de servicio `super_admin`).
- `/ready` sigue en verde.

## 4. Rollback

Con `FORCE RLS`, el owner **no** omite las políticas. Para una emergencia:

1. Preferido: usar un rol de mantenimiento con `BYPASSRLS` solo durante la
   ventana y retirarlo al terminar.
2. Alternativa: `ALTER TABLE <tabla> NO FORCE ROW LEVEL SECURITY` para las
   tablas necesarias y volver a `FORCE` al cerrar.
3. No cambiar `DATABASE_URL` al owner esperando bypass: con FORCE no lo hay.

Las políticas permanecen; no se revierten migraciones.

## 6. Ensayo de stage ejecutado (2026-11-25)

Sobre PostgreSQL/PostGIS limpio:

1. Se aplicaron todas las migraciones + `99_rls.sql` + `99z_rls_hardening.sql`.
2. Se verificó: **0 tablas sin RLS, 0 sin FORCE, 0 políticas `USING (true)`**.
3. Se ejecutó `npm run test:rls`: usuarios, pedidos, finanzas y storage
   aislados A/B; `super_admin` ve ambos.
4. Se hizo `pg_dump -Fc` de la base, `pg_restore` en una base nueva y se
   repitieron las tres consultas de cobertura y la prueba A/B: **verde**.

## 7. Despliegue real en stage

1. Backup y restore drill de stage:

   ```bash
   PGHOST=<host> PGUSER=<owner> PGPASSWORD=<pass> PGDATABASE=<db> \
     sh infra/db/restore-verify.sh /backups/<archivo>.dump
   ```

2. Promover el commit a `staging` y desplegar:

   ```bash
   gh workflow run deploy.yml --ref staging -f environment=stage
   ```

3. Verificar que `db-migrate` aplicó RLS y que `/ready` está verde.
4. Ejecutar la prueba A/B contra una **copia restaurada** de stage, no contra
   stage en caliente:

   ```bash
   RLS_TEST_DATABASE_URL=postgres://owner:...@host/rls_copy \
   RLS_TEST_APP_URL=postgres://core_engine_app:...@host/rls_copy \
   npm run test:rls
   ```

5. Smoke funcional de stage: login admin/empresario, catálogo, checkout,
   pedidos, finanzas, storage y outbox.

No aplicar RLS directamente en producción sin el paso 2 y el smoke de stage.

## 8. Límites conocidos

- Las rutas públicas de storefront (tiendas, slugs, ofertas activas) usan
  políticas públicas acotadas, no el claim de tenant.
- Los jobs y consumers deben usar `transaccion_servicio()` o el claim de
  servicio por defecto; si conectan sin claims, las tablas admin-only fallan
  cerrado.
- Las tablas de outbox/eventos son admin/servicio, no visibles para tenants.
- Las pruebas A/B cubren usuarios, pagos, comprobantes y storage; añadir una
  tabla nueva debe pasar por el barrido de `99z_rls_hardening.sql` y, si tiene
  una relación indirecta, por una política específica.
