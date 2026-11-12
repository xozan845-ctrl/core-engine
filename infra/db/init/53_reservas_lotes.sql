-- FEFO completo: asignaciones de lote por reserva de orden.
-- Permite devolver exactamente las cantidades consumidas a los lotes originales.
BEGIN;

CREATE TABLE IF NOT EXISTS catalog.reservas_lotes (
  order_id   uuid NOT NULL,
  lote_id    uuid NOT NULL REFERENCES catalog.lotes_inventario(id) ON DELETE CASCADE,
  empresa_id uuid NOT NULL,
  item_id    uuid NOT NULL,
  cantidad   integer NOT NULL CHECK (cantidad > 0),
  creado_en  timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (order_id, lote_id)
);
CREATE INDEX IF NOT EXISTS idx_reservas_lotes_empresa_item
  ON catalog.reservas_lotes (empresa_id, item_id);

COMMIT;
