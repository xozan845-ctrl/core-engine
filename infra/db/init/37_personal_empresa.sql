-- Plataforma universal · personal, roles, puestos e invitaciones por empresa.
-- Aditivo y reversible: conserva empresa_miembros e invitaciones_vendedor para
-- compatibilidad con el flujo legacy (R-DB-6).
BEGIN;

CREATE TABLE IF NOT EXISTS identity.roles_empresa (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id  uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  codigo      text NOT NULL CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre      text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  descripcion text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 500),
  permisos    jsonb NOT NULL CHECK (jsonb_typeof(permisos) = 'object'),
  activo      boolean NOT NULL DEFAULT true,
  creado_por  uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en   timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, codigo)
);

CREATE TABLE IF NOT EXISTS identity.puestos_empresa (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id  uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  codigo      text NOT NULL CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre      text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  tipo        text NOT NULL DEFAULT 'operativo' CHECK (tipo ~ '^[a-z][a-z0-9_]{1,39}$'),
  tienda_id   uuid,
  metadata    jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  activo      boolean NOT NULL DEFAULT true,
  creado_por  uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en   timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, codigo)
);

ALTER TABLE identity.empresa_miembros
  ADD COLUMN IF NOT EXISTS rol_empresa_id uuid;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresa_miembros_rol_empresa_fk'
  ) THEN
    ALTER TABLE identity.empresa_miembros
      ADD CONSTRAINT empresa_miembros_rol_empresa_fk
      FOREIGN KEY (rol_empresa_id) REFERENCES identity.roles_empresa(id);
  END IF;
  ALTER TABLE identity.empresa_miembros DROP CONSTRAINT IF EXISTS empresa_miembros_check;
  ALTER TABLE identity.empresa_miembros DROP CONSTRAINT IF EXISTS empresa_miembros_rol_check;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresa_miembros_rol_compatible_check'
  ) THEN
    ALTER TABLE identity.empresa_miembros ADD CONSTRAINT empresa_miembros_rol_compatible_check
      CHECK (rol IN ('empresario', 'vendedor', 'comprador', 'logistica', 'coordinador', 'supervisor', 'operativo')
        AND ((rol = 'vendedor' AND participacion_porcentaje IS NOT NULL)
          OR (rol <> 'vendedor' AND participacion_porcentaje IS NULL)));
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_roles_empresa_activos
  ON identity.roles_empresa (empresa_id, activo, codigo);
CREATE INDEX IF NOT EXISTS idx_puestos_empresa_tienda
  ON identity.puestos_empresa (empresa_id, tienda_id, activo);
CREATE INDEX IF NOT EXISTS idx_miembros_rol_empresa
  ON identity.empresa_miembros (empresa_id, rol_empresa_id, estado);

CREATE TABLE IF NOT EXISTS identity.asignaciones_personal (
  id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id    uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  usuario_id    uuid NOT NULL REFERENCES identity.usuarios(id) ON DELETE CASCADE,
  rol_empresa_id uuid NOT NULL REFERENCES identity.roles_empresa(id),
  tienda_id     uuid,
  puesto_id     uuid REFERENCES identity.puestos_empresa(id),
  metadata      jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  estado        text NOT NULL DEFAULT 'activa' CHECK (estado IN ('activa', 'suspendida', 'finalizada')),
  inicia_en     timestamptz NOT NULL DEFAULT NOW(),
  termina_en    timestamptz,
  creado_por    uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en     timestamptz NOT NULL DEFAULT NOW(),
  CHECK (termina_en IS NULL OR termina_en > inicia_en)
);
CREATE INDEX IF NOT EXISTS idx_asignaciones_personal_usuario
  ON identity.asignaciones_personal (empresa_id, usuario_id, estado);
CREATE INDEX IF NOT EXISTS idx_asignaciones_personal_alcance
  ON identity.asignaciones_personal (empresa_id, tienda_id, puesto_id, estado);

CREATE TABLE IF NOT EXISTS identity.invitaciones_personal (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  correo          text NOT NULL,
  nombre          text,
  tipo            text NOT NULL DEFAULT 'invitacion' CHECK (tipo IN ('invitacion', 'contratacion')),
  rol_empresa_id  uuid NOT NULL REFERENCES identity.roles_empresa(id),
  tienda_id       uuid,
  puesto_id       uuid REFERENCES identity.puestos_empresa(id),
  metadata        jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  token_hash      text NOT NULL UNIQUE,
  estado          text NOT NULL DEFAULT 'pendiente'
                  CHECK (estado IN ('pendiente', 'aceptada', 'revocada', 'expirada')),
  expira_en       timestamptz NOT NULL,
  aceptada_en     timestamptz,
  aceptada_por_usuario_id uuid REFERENCES identity.usuarios(id),
  creada_por_usuario_id   uuid NOT NULL REFERENCES identity.usuarios(id),
  creada_en       timestamptz NOT NULL DEFAULT NOW(),
  revocada_en     timestamptz
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_invitaciones_personal_pendiente
  ON identity.invitaciones_personal (empresa_id, LOWER(correo), rol_empresa_id,
    COALESCE(tienda_id, '00000000-0000-0000-0000-000000000000'::uuid),
    COALESCE(puesto_id, '00000000-0000-0000-0000-000000000000'::uuid))
  WHERE estado = 'pendiente';
CREATE INDEX IF NOT EXISTS idx_invitaciones_personal_empresa
  ON identity.invitaciones_personal (empresa_id, estado, creada_en DESC);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.roles_empresa TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.puestos_empresa TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.asignaciones_personal TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.invitaciones_personal TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.roles_empresa ENABLE ROW LEVEL SECURITY;
ALTER TABLE identity.puestos_empresa ENABLE ROW LEVEL SECURITY;
ALTER TABLE identity.asignaciones_personal ENABLE ROW LEVEL SECURITY;
ALTER TABLE identity.invitaciones_personal ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS roles_empresa_admin ON identity.roles_empresa;
DROP POLICY IF EXISTS roles_empresa_tenant ON identity.roles_empresa;
CREATE POLICY roles_empresa_admin ON identity.roles_empresa FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY roles_empresa_tenant ON identity.roles_empresa FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY roles_empresa_app ON identity.roles_empresa FOR SELECT TO core_engine_app
  USING (true);

DROP POLICY IF EXISTS puestos_empresa_admin ON identity.puestos_empresa;
DROP POLICY IF EXISTS puestos_empresa_tenant ON identity.puestos_empresa;
CREATE POLICY puestos_empresa_admin ON identity.puestos_empresa FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY puestos_empresa_tenant ON identity.puestos_empresa FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS asignaciones_personal_admin ON identity.asignaciones_personal;
DROP POLICY IF EXISTS asignaciones_personal_tenant ON identity.asignaciones_personal;
CREATE POLICY asignaciones_personal_admin ON identity.asignaciones_personal FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY asignaciones_personal_tenant ON identity.asignaciones_personal FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS invitaciones_personal_admin ON identity.invitaciones_personal;
DROP POLICY IF EXISTS invitaciones_personal_tenant ON identity.invitaciones_personal;
DROP POLICY IF EXISTS invitaciones_personal_app ON identity.invitaciones_personal;
CREATE POLICY invitaciones_personal_admin ON identity.invitaciones_personal FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY invitaciones_personal_tenant ON identity.invitaciones_personal FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
-- El endpoint público consulta por token hash y valida expiración/estado en el
-- servicio. No se expone la tabla directamente a anon/authenticated.
CREATE POLICY invitaciones_personal_app ON identity.invitaciones_personal FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

COMMIT;
