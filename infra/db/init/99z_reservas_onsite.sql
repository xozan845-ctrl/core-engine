-- Las verticales de servicios (belleza/dental) necesitan `onsite` para que el
-- checkout de reserva acepte la cita. Es una capacidad de fulfillment, no un
-- cambio de código por rubro.
BEGIN;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
VALUES
  ('belleza', 'fulfillment_onsite', '{}'::jsonb, true),
  ('dental', 'fulfillment_onsite', '{}'::jsonb, true)
ON CONFLICT (vertical_codigo, capacidad_codigo) DO UPDATE
  SET config = EXCLUDED.config, activo = true;

COMMIT;
