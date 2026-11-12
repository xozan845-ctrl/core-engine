-- Endurecimiento de concurrencia (auditoría 2026-11-08).
-- 1) Outbox: reclamo atómico de filas para evitar doble publicación entre
--    ticks solapados o réplicas. `bloqueado_en` funciona como lease: si el
--    proceso muere, la fila se puede reclamar tras 5 minutos.
-- 2) Event store: la versión del agregado debe ser única para que la
--    concurrencia optimista no acepte dos eventos con la misma versión.

DO $$
DECLARE
  r record;
BEGIN
  FOR r IN
    SELECT schemaname, tablename
      FROM pg_tables
     WHERE tablename = 'outbox'
       AND schemaname NOT IN ('pg_catalog', 'information_schema')
  LOOP
    EXECUTE format(
      'ALTER TABLE %I.%I ADD COLUMN IF NOT EXISTS bloqueado_en timestamptz',
      r.schemaname, r.tablename
    );
    EXECUTE format(
      'CREATE INDEX IF NOT EXISTS %I ON %I.%I (estado, creado_en) WHERE estado = ''pendiente''',
      'idx_' || r.schemaname || '_outbox_claim', r.schemaname, r.tablename
    );
  END LOOP;
END $$;

DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM orders.eventos
     GROUP BY aggregate_id, version
    HAVING COUNT(*) > 1
  ) THEN
    RAISE EXCEPTION
      'orders.eventos tiene versiones duplicadas por agregado; resuélvelas antes de aplicar el índice único';
  END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS uq_orders_eventos_aggregate_version
  ON orders.eventos (aggregate_id, version);

-- 3) Contabilidad: cuenta para la parte del empresario, que antes se omitía
--    del asiento de realización (el pasivo quedaba subestimado).
INSERT INTO finance.plan_cuentas (codigo, nombre, tipo, naturaleza, nivel)
VALUES ('2.1.6', 'Acreedores empresarios (comisiones por liquidar)', 'PASIVO', 'ACREEDORA', 3)
ON CONFLICT (codigo) DO NOTHING;
