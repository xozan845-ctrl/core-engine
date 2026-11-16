-- Fase 2 · las suscripciones aceptan cualquier método activo del catálogo,
-- no solo simulado/credito. Se elimina el CHECK fijo y se usa la FK.
BEGIN;

DO $$
DECLARE restriccion record;
BEGIN
  FOR restriccion IN
    SELECT conname
      FROM pg_constraint
     WHERE conrelid = 'orders.suscripciones'::regclass
       AND contype = 'c'
       AND pg_get_constraintdef(oid) LIKE '%metodo_pago%'
  LOOP
    EXECUTE format('ALTER TABLE orders.suscripciones DROP CONSTRAINT %I', restriccion.conname);
  END LOOP;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'suscripciones_metodo_pago_fk'
  ) THEN
    ALTER TABLE orders.suscripciones
      ADD CONSTRAINT suscripciones_metodo_pago_fk
      FOREIGN KEY (metodo_pago) REFERENCES orders.metodos_pago(codigo);
  END IF;
END $$;

COMMIT;
