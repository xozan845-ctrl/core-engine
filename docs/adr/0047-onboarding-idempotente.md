# ADR-0047 — Onboarding idempotente de empresa

- **Fecha:** 2026-11-20
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0021](./0021-negocios-por-empresa.md),
  [ADR-0033](./0033-idempotencia-http.md),
  [ADR-0046](./0046-suscripcion-plataforma-expansion.md)

## Contexto

El alta de una empresa ya era una sola transacción (`POST /empresas`): crea
empresa, negocio `principal`, pack, capacidades, roles, ubicación de inventario
y asigna al usuario como empresario. Sin embargo, como producto faltaba:

- una `Idempotency-Key` para que un timeout o doble clic no duplicara empresas;
- un estado consultable si la respuesta se pierde;
- un token con el `tenant_id` nuevo (el access token anterior queda viejo);
- una respuesta consolidada para el frontend.

El análisis proponía un endpoint público que también creara al admin. Eso
permitiría altas sin verificación de correo y con un admin arbitrario, así que
se descartó para el MVP.

## Decisión

1. **Onboarding autenticado.**
   - `POST /api/v2/onboarding` exige un `empresario` autenticado. El actor es el
     admin; no se crean usuarios ni se acepta un `admin` externo.
   - El registro/invitación del admin sigue siendo un flujo aparte.
2. **Idempotencia obligatoria.**
   - `Idempotency-Key` es obligatoria (8–255 caracteres).
   - El hash canónico del cuerpo se guarda en `identity.provisioning_jobs`
     junto con el actor y la clave (única por actor).
   - Misma clave + mismo cuerpo: replay con `Idempotency-Replayed: true`.
   - Misma clave + otro cuerpo: `422 IDEMPOTENCIA_CLAVE_REUTILIZADA`.
   - Clave en proceso: `409 IDEMPOTENCIA_EN_PROCESO`.
3. **Estado y recuperación.**
   - El job tiene `en_proceso|completado|fallido`, paso, empresa/negocio,
     resultado y error.
   - `GET /api/v2/onboarding/{id}` devuelve el estado; el frontend puede
     consultarlo tras un timeout.
   - Un job `en_proceso` con más de 2 minutos puede reclamarse. Si el commit de
     la empresa ocurrió pero se perdió la respuesta, el reintento adopta la
     empresa existente por `slug + creado_por` en vez de duplicarla.
4. **Slug determinista.**
   - Se usa `empresa.slug` o `dominio_deseado` como slug.
   - Sin slug, se deriva del nombre y de un hash de la clave, por lo que el
     reintento usa el mismo slug.
   - Los dominios personalizados (`mi-salon.com`) no están implementados:
     devuelven `DOMINIO_NO_SOPORTADO`.
5. **Respuesta consolidada.**
   - `empresa`, `negocio_id`, `slug`, `capacidades`, `tema` resuelto y un
     `access_token` nuevo con el tenant. También incluye `onboarding_id`.
   - `storage` se informa como `lazy`: el bucket se crea en la primera subida o
     en `GET /api/v2/storage/bucket`; no se orquesta storage en esta transacción.
6. **Plan de plataforma.**
   - Antes de crear se aplica la regla de expansión de ADR-0046. Un replay de un
     job completado no vuelve a pasar por caja.
7. **Tema personalizado.**
   - `Empresas::crear` ahora persiste `plantilla_override`/`tema_override` del
     perfil de alta; antes se validaban pero se ignoraban.
8. **Fuera de alcance explícito.**
   - `datos_demo`, invitaciones de personal y pre-provisión de bucket/dominio.
     Se documentan en [`docs/backlog-plataforma.md`](../backlog-plataforma.md).

## Consecuencias

- **Positivas:** el frontend hace una llamada, recibe todo lo necesario y puede
  reintentar sin duplicar. El token nuevo evita el re-login obligatorio.
- **Seguridad:** no hay alta pública ni creación de usuarios sin verificación;
  el endpoint queda detrás del gateway y del rol `empresario`.
- **Estado:** la creación es síncrona; las extensiones (bucket, correo,
  dominio) necesitarán un job asíncrono con pasos, no este endpoint.
- **Datos:** `provisioning_jobs` crece con cada alta; la limpieza/retensión es
  una tarea de operación pendiente (ver backlog).

## Criterios de aceptación

- Dos `POST /onboarding` con la misma clave y cuerpo devuelven la misma empresa
  y no crean una segunda.
- Misma clave con otro cuerpo devuelve 422; sin clave devuelve 400.
- El `access_token` devuelto lleva el `tenant_id` de la empresa nueva.
- Un reintento tras un fallo recupera la empresa por slug en vez de duplicarla.
- Un segundo onboarding sin plan devuelve `402 SUSCRIPCION_REQUERIDA`.
- `GET /onboarding/{id}` devuelve `completado`, `en_proceso` o `fallido`.
