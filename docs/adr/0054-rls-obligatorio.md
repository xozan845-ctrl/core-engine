# ADR-0054 — RLS obligatorio y efectivo en todos los entornos

- **Fecha:** 2026-11-25
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0035](./0035-eliminacion-tenant.md),
  [ADR-0053](./0053-cobro-centralizado-dispersion-diaria.md)

## Contexto

`docker-compose*.yml` desplegaba `db-migrate` con `APPLY_RLS=false`, por lo que
`infra/db/supabase/99_rls.sql` no se aplicaba en stage/producción. El rol
`core_engine_app` era `NOBYPASSRLS`, pero muchas tablas tenían políticas
`TO core_engine_app USING (true)` que actuaban como bypass efectivo: el mismo
rol que atiende las peticiones HTTP podía leer y escribir cualquier tenant.
La auditoría midió ~95 tablas de aplicación sin RLS, incluidas tablas sensibles.

## Decisión

1. **RLS obligatorio.** `apply.sh` aplica siempre `99_rls.sql` y
   `99z_rls_hardening.sql`, después de las migraciones y registrados en
   `public.schema_migrations`. Ya no existe `APPLY_RLS=false`.
2. **Idempotencia.** Cada `CREATE POLICY` de `99_rls.sql` va precedido de
   `DROP POLICY IF EXISTS`, para convivir con las políticas creadas por las
   migraciones de init y poder reaplicarse sin drift.
3. **Cambios incrementales versionados.** `99_rls.sql` y
   `99z_rls_hardening.sql` son inmutables una vez aplicados. Cualquier ajuste
   posterior de RLS va en un archivo nuevo
   `infra/db/supabase/migrations/AAAAMMDD_NN_tema.sql`, que `apply.sh` ejecuta
   en orden lexicográfico y registra con checksum. Así una base persistente
   (stage/prod) actualiza sin *drift* y cada cambio queda auditable.
4. **Sin bypass del rol de app.** Se eliminan las políticas
   `TO core_engine_app USING (true)`. `core_engine_app` queda
   `NOSUPERUSER NOBYPASSRLS`; los jobs usan el claim de servicio
   `super_admin` que ya inyectan las conexiones.
5. **Sin `USING (true)`.** Las lecturas públicas de storefront usan predicados
   concretos (`estado = 'activa'`, `activo`, `id = 1`, etc.). Las tablas de
   outbox/eventos usan una política admin/servicio.
6. **FORCE RLS.** Todas las tablas de aplicación con RLS quedan con
   `FORCE ROW LEVEL SECURITY`. El owner tampoco omite las políticas; las
   migraciones se ejecutan con claim `super_admin` y el backup usa un rol con
   `BYPASSRLS` (documentado en el runbook).
7. **Cobertura dinámica.** `99z_rls_hardening.sql` recorre las tablas de
   aplicación, habilita RLS donde falte y crea:
   - política de tenant por `empresa_id`, `tenant_id` o `negocio_id`;
   - política admin-only si la tabla no tiene columna de tenant;
   - políticas específicas para usuarios, sesiones, timeline de pedidos,
     asientos y series fiscales.
8. **Pruebas negativas A/B.** CI tiene un job dedicado con PostgreSQL/PostGIS
   que aplica todas las migraciones + RLS dos veces (idempotencia) y ejecuta
   `scripts/tools/rls-ab-test.mjs`: usuarios, pagos, comprobantes y objetos de
   storage de A no son visibles ni modificables desde B; `super_admin` ve ambos.

## Consecuencias

- **Positivas:** el aislamiento ya no depende solo de los `WHERE empresa_id`;
  un fallo de query no filtra filas de otro tenant. Toda tabla nueva queda
  cubierta por el barrido dinámico.
- **Datos:** `99_rls.sql` idempotente, nuevo `99z_rls_hardening.sql`, ledger de
  migraciones actualizado. Los cambios posteriores viven en
  `infra/db/supabase/migrations/`; el manifiesto
  `scripts/config/db-migrations.json` cubre ambos directorios.
- **Operación:** un owner sin `BYPASSRLS` ya no puede saltarse RLS por ser
  owner. Las tareas de mantenimiento deben usar claims de servicio o un rol
  `BYPASSRLS` explícito.
- **Rendimiento:** las políticas añaden subconsultas en tablas de join
  (`orden_timeline`, `asientos`). Los índices por `empresa_id`, `tenant_id` y
  `order_id` ya existen; se mide en staging.
- **Compatibilidad:** las rutas públicas de storefront siguen leyendo tiendas,
  slugs y ofertas activas; las políticas de app se sustituyen por admin/servicio.

## Criterios de aceptación

- `pg_tables` de esquemas de aplicación: **0 tablas sin RLS**.
- `pg_class`: **0 tablas con RLS sin FORCE**.
- `pg_policies`: **0 políticas `USING (true)`** en esquemas de aplicación.
- `SELECT`/`UPDATE`/`DELETE` de A sobre filas de B devuelve 0 filas o error.
- `super_admin` conserva visibilidad global.
- `npm run test:rls` verde en CI sobre una base recién migrada.
