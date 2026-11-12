-- Fase 3 · preparación para 43_catalog_item_model.sql (R-DB-6).
-- 43 usa CREATE OR REPLACE VIEW y agrega columnas en medio de
-- catalog.items_vista; PostgreSQL solo permite añadirlas al final. Se retira
-- la vista para que 43 la recree con el contrato de variantes; 44 la vuelve a
-- crear con los metadatos finales.
BEGIN;
DROP VIEW IF EXISTS catalog.items_vista;
COMMIT;
