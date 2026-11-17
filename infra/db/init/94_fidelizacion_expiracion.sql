-- Fase 8 · fidelización: soporte para la expiración de puntos inactivos.
-- Índice parcial para recorrer tarjetas activas por programa sin escanear la
-- tabla completa en cada pasada del scheduler.
BEGIN;

CREATE INDEX IF NOT EXISTS idx_fidelizacion_tarjetas_programa_activa
  ON orders.fidelizacion_tarjetas (programa_id)
  WHERE estado = 'activa' AND puntos > 0;

COMMIT;
