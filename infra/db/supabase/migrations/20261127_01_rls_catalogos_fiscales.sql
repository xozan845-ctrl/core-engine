-- RLS: catálogos fiscales de plataforma legibles por los tenants (ADR-0054).
--
-- Los necesita el onboarding para resolver región y régimen antes de crear la
-- empresa. Sin lectura, el pack parece no declarar regiones.
BEGIN;

ALTER TABLE finance.jurisdicciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS jurisdicciones_lectura ON finance.jurisdicciones;
CREATE POLICY jurisdicciones_lectura ON finance.jurisdicciones
  FOR SELECT TO anon, authenticated USING (activa);

ALTER TABLE finance.regimenes_fiscales ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS regimenes_fiscales_lectura ON finance.regimenes_fiscales;
CREATE POLICY regimenes_fiscales_lectura ON finance.regimenes_fiscales
  FOR SELECT TO anon, authenticated USING (estado = 'activo');

COMMIT;
