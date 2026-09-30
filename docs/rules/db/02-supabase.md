# 02 — Reglas Doradas de Supabase

Aplica a: paridad local/producción, RLS, Auth, migraciones y secretos cuando
Core Engine corre sobre Supabase (staging/producción). Local (Docker) usa una
imagen Postgres estándar compatible con el ecosistema Supabase.

## Reglas

| ID | Regla |
|---|---|
| R-SB-1 | **Paridad de entornos (local vs. producción)**: el entorno local (`docker-compose.yml`) levanta una imagen de Postgres estándar. Sin embargo, toda la estructura de la base de datos **DEBE** ser compatible con el ecosistema de Supabase. Las configuraciones específicas de Supabase (como Auth, Storage y RLS avanzado) que no están en el contenedor local básico deben simularse o documentarse claramente en los scripts de inicialización. |
| R-SB-2 | **Row Level Security (RLS) obligatorio**: toda tabla creada en los esquemas transaccionales (`orders`, `stores`, `identity`, etc.) **DEBE** tener RLS habilitado (`ALTER TABLE nombre_tabla ENABLE ROW LEVEL SECURITY;`). El acceso a los datos se debe gobernar estrictamente a través de políticas (policies) que evalúen el rol o ID del usuario autenticado (`auth.uid()`, `auth.jwt() ->> 'rol'`). Las reglas RLS deben estar definidas y versionadas en el archivo `infra/db/supabase/99_rls.sql` (o en su respectiva migración) para ser aplicadas en producción. |
| R-SB-3 | **Uso de funciones nativas de Supabase Auth**: aunque el Gateway emite y valida JWTs (HS256) compatibles con Supabase para mantener el control local, en producción las firmas de los tokens deben coincidir exactamente con el `SUPABASE_JWT_SECRET`. Dado que el Gateway centraliza la autenticación e inyecta la identidad en los headers (`x-user-id`, `x-user-rol`), cuando un microservicio se conecta a Postgres **debe usar esos headers** para inyectar el contexto de la petición en la sesión de base de datos (ej: ejecutando `set_config('request.jwt.claims', '{"sub":"id", "rol":"rol"}', true)`) para que el motor RLS de Supabase pueda evaluar las políticas. |
| R-SB-4 | **Gestión de migraciones**: ver **`R-DB-6` (`01-database.md`)** — jerarquía de fuente de verdad, canónica para todo lo relacionado con migraciones. Resumen aplicable a Supabase: está prohibido realizar cambios estructurales directamente en el Dashboard web de Supabase en producción. Usar exclusivamente `supabase db push` con scripts SQL versionados. |
| R-SB-5 | **Aislamiento de secretos de Supabase**: las variables como `SUPABASE_URL`, `SUPABASE_ANON_KEY` y `SUPABASE_SERVICE_ROLE_KEY` nunca deben quemarse en el código (R-DS-1). El `SERVICE_ROLE_KEY` solo debe usarse en scripts de infraestructura interna o tareas administrativas de alto nivel (como *background jobs* o migraciones), jamás en microservicios que atienden peticiones HTTP de usuarios comunes, ya que este bypass **ignora el RLS**. |
| R-SB-6 | **Desacoplamiento de Supabase Storage / Edge Functions (opcional pero recomendado)**: si el proyecto comienza a usar Supabase Storage (para imágenes de productos, etc.), el acceso debe abstraerse detrás de una interfaz en `packages/shared/`, de modo que el negocio no esté fuertemente acoplado al SDK de Supabase, facilitando las pruebas unitarias. |
