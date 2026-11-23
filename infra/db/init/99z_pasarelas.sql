-- Pasarelas concretas detrás de la ACL ProveedorPagos.
-- No contiene credenciales: solo nombres de variables de entorno y URLs base
-- configurables. El admin activa el proveedor y el tenant lo selecciona.
-- El método `tarjeta` nace inactivo para que nadie cobre con el simulado por
-- error; se activa después de configurar una pasarela real.
BEGIN;

-- El catalogo original solo permitia simulado/http/manual (99_pagos_configurables).
-- Se amplia antes de sembrar los proveedores concretos.
ALTER TABLE orders.proveedores_pago
  DROP CONSTRAINT IF EXISTS proveedores_pago_tipo_check;
ALTER TABLE orders.proveedores_pago
  ADD CONSTRAINT proveedores_pago_tipo_check
  CHECK (tipo IN ('simulado', 'http', 'manual', 'tilopay', 'pagadito'));

ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS referencia_proveedor text NOT NULL DEFAULT '';

INSERT INTO orders.proveedores_pago (codigo, nombre, tipo, activo, config)
VALUES
  ('tilopay', 'Tilopay', 'tilopay', false,
   '{"base_url":"https://app.tilopay.com","apiuser_env":"TILOPAY_APIUSER","password_env":"TILOPAY_PASSWORD","key_env":"TILOPAY_KEY"}'::jsonb),
  ('pagadito', 'Pagadito', 'pagadito', false,
   '{"base_url":"https://comercios.pagadito.com/apipg/charges.php","uid_env":"PAGADITO_UID","wsk_env":"PAGADITO_WSK","currency":"USD"}'::jsonb)
ON CONFLICT (codigo) DO UPDATE
  SET nombre = EXCLUDED.nombre,
      tipo = EXCLUDED.tipo,
      actualizado_en = NOW();

INSERT INTO orders.metodos_pago (codigo, nombre, requiere_confirmacion, activo, config)
VALUES ('tarjeta', 'Tarjeta (pasarela)', false, false, '{"cargo_externo":true}'::jsonb)
ON CONFLICT (codigo) DO UPDATE
  SET nombre = EXCLUDED.nombre,
      requiere_confirmacion = EXCLUDED.requiere_confirmacion,
      config = EXCLUDED.config,
      actualizado_en = NOW();

COMMIT;
