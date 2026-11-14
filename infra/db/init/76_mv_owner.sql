-- Las vistas materializadas las crea el rol dueño del init (core_engine /
-- postgres), pero `market-intelligence-service` refresca con `core_engine_app`
-- y `REFRESH MATERIALIZED VIEW CONCURRENTLY` exige ser dueño. Sin esto el
-- refresco falla cada ciclo ("must be owner of materialized view").
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    ALTER MATERIALIZED VIEW IF EXISTS intelligence.mv_resumen_ejecutivo
      OWNER TO core_engine_app;
    ALTER MATERIALIZED VIEW IF EXISTS intelligence.mv_tendencias
      OWNER TO core_engine_app;
  END IF;
END $$;
