-- ADR-0018 · retiro definitivo del esquema `catalog`.
--
-- Cierra la ventana de rollback del corte: los datos viven en `item`,
-- `pricing` e `inventory`, y las referencias externas ya apuntan a
-- `item.items` (migración `83_item_fk_externas`).
--
-- Antes de borrar se comprueba que ninguna FK ni vista de otro esquema dependa
-- de `catalog`. Si aparece una dependencia, la migración falla para resolverla
-- explícitamente en vez de arrastrarla con `CASCADE`.

BEGIN;

DO $$
DECLARE
  v_fks text;
  v_vistas text;
  v_dependencias text;
BEGIN
  SELECT string_agg(DISTINCT format('%s.%s', n.nspname, c.relname), ', ')
    INTO v_fks
    FROM pg_constraint fk
    JOIN pg_class c ON c.oid = fk.conrelid
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_class t ON t.oid = fk.confrelid
    JOIN pg_namespace tn ON tn.oid = t.relnamespace
   WHERE fk.contype = 'f'
     AND tn.nspname = 'catalog'
     AND n.nspname <> 'catalog';

  SELECT string_agg(DISTINCT format('%s.%s', n.nspname, c.relname), ', ')
    INTO v_vistas
    FROM pg_depend d
    JOIN pg_rewrite r ON r.oid = d.objid
    JOIN pg_class c ON c.oid = r.ev_class
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_class t ON t.oid = d.refobjid
    JOIN pg_namespace tn ON tn.oid = t.relnamespace
   WHERE d.classid = 'pg_rewrite'::regclass
     AND d.refclassid = 'pg_class'::regclass
     AND tn.nspname = 'catalog'
     AND n.nspname <> 'catalog';

  v_dependencias := NULLIF(concat_ws(', ', v_fks, v_vistas), '');

  IF v_dependencias IS NOT NULL THEN
    RAISE EXCEPTION 'CATALOG_CON_DEPENDENCIAS: %', v_dependencias;
  END IF;
END $$;

DROP SCHEMA catalog CASCADE;

COMMIT;
