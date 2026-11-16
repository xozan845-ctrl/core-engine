-- ADR-0018 · referencias externas al catalogo maestro.
--
-- `81_item_schema` copia los items a `item.items`, pero las tablas de otros
-- esquemas conservaban la FK a `catalog.items` (hoy `stores.ofertas.item_id`).
-- Con el corte, los items nuevos solo existen en `item.items`, asi que la FK
-- vieja rechazaba ofertas y referencias legitimas con un 500.
--
-- Repunta esas FK a `item.items`. Es idempotente: si ya apunta al esquema
-- nuevo, no hace nada. El rollback vuelve a apuntarla a `catalog.items`.

DO $$
BEGIN
  IF to_regclass('stores.ofertas') IS NOT NULL
     AND EXISTS (
       SELECT 1 FROM pg_constraint
        WHERE conname = 'ofertas_item_id_fkey'
          AND conrelid = 'stores.ofertas'::regclass
          AND confrelid = 'catalog.items'::regclass
     ) THEN
    ALTER TABLE stores.ofertas DROP CONSTRAINT ofertas_item_id_fkey;
    ALTER TABLE stores.ofertas
      ADD CONSTRAINT ofertas_item_id_fkey
      FOREIGN KEY (item_id) REFERENCES item.items(id) NOT VALID;
    ALTER TABLE stores.ofertas VALIDATE CONSTRAINT ofertas_item_id_fkey;
  END IF;
END $$;
