-- Fase 7: el tipo de orden deja de tener default legacy.
-- Los productores deben enviar `tipo_orden` explícito en OrderCreatedEvent.
BEGIN;

ALTER TABLE orders.orden_vista
  ALTER COLUMN tipo_orden DROP DEFAULT;

COMMIT;
