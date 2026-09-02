# Reglas Doradas de Supabase (Supabase)

Dado que Core Engine utiliza una instancia de Postgres compatible con Supabase localmente (vía Docker) y se apoya en la plataforma Supabase para staging/producción, se deben seguir reglas estrictas para garantizar la paridad de entornos y la seguridad.

1. **Paridad de Entornos (Local vs. Producción)**:
   - El entorno local (`docker-compose.yml`) levanta una imagen de Postgres estándar. Sin embargo, toda la estructura de la base de datos **DEBE** ser compatible con el ecosistema de Supabase.
   - Las configuraciones específicas de Supabase (como Auth, Storage y RLS avanzado) que no están en el contenedor local básico deben simularse o documentarse claramente en los scripts de inicialización.

2. **Row Level Security (RLS) Obligatorio**:
   - Toda tabla creada en los esquemas transaccionales (`orders`, `stores`, `identity`, etc.) **DEBE** tener RLS habilitado (`ALTER TABLE nombre_tabla ENABLE ROW LEVEL SECURITY;`).
   - El acceso a los datos se debe gobernar estrictamente a través de políticas (Policies) que evalúen el rol o ID del usuario autenticado (`auth.uid()`, `auth.jwt() ->> 'rol'`).
   - Las reglas RLS deben estar definidas y versionadas en el archivo `infra/db/supabase/99_rls.sql` (o en su respectiva migración) para ser aplicadas en producción.

3. **Uso de Funciones Nativas de Supabase Auth**:
   - Aunque el Gateway emite y valida JWTs (HS256) compatibles con Supabase para mantener el control local, en producción las firmas de los tokens deben coincidir exactamente con el `SUPABASE_JWT_SECRET`.
   - Dado que el Gateway centraliza la autenticación e inyecta la identidad en los Headers (`x-user-id`, `x-user-rol`), cuando un microservicio se conecta a Postgres **debe usar esos Headers** para inyectar el contexto de la petición en la sesión de base de datos (Ej: ejecutando `set_config('request.jwt.claims', '{"sub":"id", "rol":"rol"}', true)`) para que el motor RLS de Supabase pueda evaluar las políticas.

4. **Gestión de Migraciones**: Ver **`database.md` Regla 6 (Jerarquía de Fuente de Verdad)**. Esta es la fuente canónica para todo lo relacionado con migraciones. Resumen aplicable a Supabase: está prohibido realizar cambios estructurales directamente en el Dashboard web de Supabase en producción. Usar exclusivamente `supabase db push` con scripts SQL versionados.

5. **Aislamiento de Secretos de Supabase**:
   - Las variables como `SUPABASE_URL`, `SUPABASE_ANON_KEY` y `SUPABASE_SERVICE_ROLE_KEY` nunca deben quemarse en el código.
   - El `SERVICE_ROLE_KEY` solo debe usarse en scripts de infraestructura interna o tareas administrativas de alto nivel (como *background jobs* o migraciones), jamás en microservicios que atienden peticiones HTTP de usuarios comunes, ya que este bypass(ignora) el RLS.

6. **Desacoplamiento de Supabase Storage / Edge Functions (Opcional pero recomendado)**:
   - Si el proyecto comienza a usar Supabase Storage (para imágenes de productos, etc.), el acceso debe abstraerse detrás de una interfaz en `packages/shared/`, de modo que el negocio no esté fuertemente acoplado al SDK de Supabase, facilitando las pruebas unitarias.
