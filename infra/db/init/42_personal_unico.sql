-- Fase 5: personal universal como única fuente de membresías.
-- Migra empresa_miembros a asignaciones_personal y retira el flujo legacy
-- de invitaciones de vendedores.
BEGIN;

ALTER TABLE identity.asignaciones_personal
  ADD COLUMN IF NOT EXISTS participacion_porcentaje integer
    CHECK (participacion_porcentaje IS NULL OR participacion_porcentaje BETWEEN 0 AND 100);

-- Roles base por empresa para poder migrar las membresías existentes.
INSERT INTO identity.roles_empresa (empresa_id, codigo, nombre, permisos, creado_por)
SELECT e.id, r.codigo, r.nombre, '{}'::jsonb, e.creado_por
  FROM identity.empresas e
 CROSS JOIN (VALUES
   ('empresario', 'Empresario'),
   ('vendedor', 'Vendedor')
 ) AS r(codigo, nombre)
ON CONFLICT (empresa_id, codigo) DO NOTHING;

INSERT INTO identity.asignaciones_personal
  (empresa_id, usuario_id, rol_empresa_id, participacion_porcentaje,
   estado, creado_por, creado_en)
SELECT m.empresa_id, m.usuario_id, r.id, m.participacion_porcentaje,
       CASE m.estado
         WHEN 'activo' THEN 'activa'
         WHEN 'suspendido' THEN 'suspendida'
         ELSE 'finalizada'
       END,
       e.creado_por, m.creado_en
  FROM identity.empresa_miembros m
  JOIN identity.empresas e ON e.id = m.empresa_id
  JOIN identity.roles_empresa r
    ON r.empresa_id = m.empresa_id AND r.codigo = m.rol
 WHERE NOT EXISTS (
   SELECT 1 FROM identity.asignaciones_personal a
    WHERE a.empresa_id = m.empresa_id AND a.usuario_id = m.usuario_id
 );

DROP TABLE IF EXISTS identity.invitaciones_vendedor CASCADE;
DROP TABLE IF EXISTS identity.empresa_miembros CASCADE;

COMMIT;
