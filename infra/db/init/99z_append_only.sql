-- Append-only real para el event store de orders y la contabilidad.
-- El rol de aplicación conserva INSERT/SELECT, pero pierde UPDATE/DELETE y
-- cualquier mutación directa falla en la base, no solo por convención.
BEGIN;

CREATE OR REPLACE FUNCTION public.rechazar_mutacion_append_only()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  RAISE EXCEPTION 'La tabla % es append-only', TG_TABLE_NAME;
END;
$$;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    REVOKE UPDATE, DELETE ON orders.eventos FROM core_engine_app;
    REVOKE UPDATE, DELETE ON finance.asientos FROM core_engine_app;
    REVOKE UPDATE, DELETE ON finance.asiento_detalles FROM core_engine_app;
  END IF;
END $$;

DROP TRIGGER IF EXISTS eventos_append_only ON orders.eventos;
CREATE TRIGGER eventos_append_only
  BEFORE UPDATE OR DELETE ON orders.eventos
  FOR EACH ROW EXECUTE FUNCTION public.rechazar_mutacion_append_only();

DROP TRIGGER IF EXISTS asientos_append_only ON finance.asientos;
CREATE TRIGGER asientos_append_only
  BEFORE UPDATE OR DELETE ON finance.asientos
  FOR EACH ROW EXECUTE FUNCTION public.rechazar_mutacion_append_only();

DROP TRIGGER IF EXISTS asiento_detalles_append_only ON finance.asiento_detalles;
CREATE TRIGGER asiento_detalles_append_only
  BEFORE UPDATE OR DELETE ON finance.asiento_detalles
  FOR EACH ROW EXECUTE FUNCTION public.rechazar_mutacion_append_only();

COMMIT;
