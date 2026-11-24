-- Proveedores de pago y selección por empresa/negocio (ADR-0037).
--
-- El dominio orders sigue hablando con la ACL `ProveedorPagos`; la
-- configuración decide qué adaptador se instancia. Las credenciales no se
-- guardan en la tabla: `config.clave_env` referencia una variable de entorno.
BEGIN;

CREATE TABLE IF NOT EXISTS orders.proveedores_pago (
  codigo      text PRIMARY KEY,
  nombre      text NOT NULL,
  tipo        text NOT NULL CHECK (tipo IN ('simulado', 'http', 'manual')),
  activo      boolean NOT NULL DEFAULT true,
  config      jsonb NOT NULL DEFAULT '{}'::jsonb
              CHECK (jsonb_typeof(config) = 'object'),
  creado_en   timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO orders.proveedores_pago (codigo, nombre, tipo, activo, config)
VALUES
  ('simulado', 'Simulado', 'simulado', true, '{}'::jsonb),
  ('http', 'Pasarela HTTP genérica', 'http', false, '{}'::jsonb),
  ('manual', 'Cobro manual', 'manual', true, '{}'::jsonb)
ON CONFLICT (codigo) DO UPDATE
  SET nombre = EXCLUDED.nombre,
      tipo = EXCLUDED.tipo,
      actualizado_en = NOW();

CREATE TABLE IF NOT EXISTS orders.configuracion_pagos (
  id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id       uuid NOT NULL,
  negocio_id       uuid,
  proveedor_codigo text NOT NULL REFERENCES orders.proveedores_pago(codigo),
  activo           boolean NOT NULL DEFAULT true,
  config           jsonb NOT NULL DEFAULT '{}'::jsonb
                   CHECK (jsonb_typeof(config) = 'object'),
  creado_en        timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en   timestamptz NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_configuracion_pagos_empresa
  ON orders.configuracion_pagos (empresa_id)
  WHERE negocio_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_configuracion_pagos_negocio
  ON orders.configuracion_pagos (empresa_id, negocio_id)
  WHERE negocio_id IS NOT NULL;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON orders.proveedores_pago TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON orders.configuracion_pagos TO core_engine_app;
  END IF;
END $$;

COMMIT;
