-- RLS: onboarding del empresario (ADR-0054).
--
-- 1) El creador lee la empresa que acaba de insertar: `INSERT ... RETURNING`
--    exige política SELECT sobre la fila nueva y su token aún no tiene tenant.
-- 2) El creador vincula su propia fila (`tenant_id IS NULL`) con la empresa que
--    creó. Sin esto el UPDATE afecta 0 filas en silencio y el token reemitido
--    no lleva tenant.
BEGIN;

DROP POLICY IF EXISTS empresas_miembros ON identity.empresas;
CREATE POLICY empresas_miembros ON identity.empresas FOR SELECT TO authenticated
  USING (id::text = (auth.jwt() ->> 'tenant_id')
         OR (auth.rol() = 'empresario' AND creado_por = auth.uid()));

DROP POLICY IF EXISTS usuarios_onboarding_tenant ON identity.usuarios;
CREATE POLICY usuarios_onboarding_tenant ON identity.usuarios FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario'
         AND id = auth.uid()
         AND tenant_id IS NULL)
  WITH CHECK (auth.rol() = 'empresario'
              AND EXISTS (
                    SELECT 1 FROM identity.empresas e
                     WHERE e.id = tenant_id
                       AND e.creado_por = auth.uid()
                  ));

COMMIT;
