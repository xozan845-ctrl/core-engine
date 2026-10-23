-- Invitaciones de vendedores por empresa y tiendas disponibles.
-- Aditivo para bases existentes (R-DB-6): no se editan migraciones anteriores.
BEGIN;

-- Una tienda puede existir sin vendedor asignado; al aceptar una invitacion se
-- asigna el vendedor y pasa a ser su tienda operativa (ADR-25).
ALTER TABLE stores.tiendas ALTER COLUMN vendedor_id DROP NOT NULL;

-- Un vendedor solo puede tener una tienda; las tiendas sin vendedor no cuentan.
CREATE UNIQUE INDEX IF NOT EXISTS idx_tiendas_vendedor_unico
  ON stores.tiendas (vendedor_id)
  WHERE vendedor_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS identity.invitaciones_vendedor (
  id                        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id                uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  tienda_id                 uuid NOT NULL REFERENCES stores.tiendas(id) ON DELETE CASCADE,
  correo                    text NOT NULL,
  nombre                    text,
  participacion_porcentaje  integer NOT NULL CHECK (participacion_porcentaje BETWEEN 0 AND 100),
  token_hash                text NOT NULL UNIQUE,
  estado                    text NOT NULL DEFAULT 'pendiente'
                            CHECK (estado IN ('pendiente', 'aceptada', 'revocada', 'expirada')),
  expira_en                 timestamptz NOT NULL,
  aceptada_en               timestamptz,
  aceptada_por_usuario_id   uuid REFERENCES identity.usuarios(id),
  creada_por_usuario_id     uuid NOT NULL REFERENCES identity.usuarios(id),
  creada_en                 timestamptz NOT NULL DEFAULT NOW(),
  revocada_en               timestamptz
);

-- Evita dos invitaciones pendientes para el mismo correo/empresa/tienda.
CREATE UNIQUE INDEX IF NOT EXISTS idx_invitaciones_pendientes_unicas
  ON identity.invitaciones_vendedor (empresa_id, tienda_id, LOWER(correo))
  WHERE estado = 'pendiente';

CREATE INDEX IF NOT EXISTS idx_invitaciones_empresa
  ON identity.invitaciones_vendedor (empresa_id, estado, creada_en DESC);

CREATE INDEX IF NOT EXISTS idx_invitaciones_token
  ON identity.invitaciones_vendedor (token_hash);

COMMIT;
