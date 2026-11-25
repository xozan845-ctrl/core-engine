# identity-service · identidad, tenancy y capacidades

`identity-service` es la **autoridad de identidad y tenancy** de Core Engine.
Emite y revoca sesiones (JWT HS256), administra empresas y negocios, resuelve
las **capacidades** efectivas de cada tenant, publica el **manifiesto de
features**, gestiona personal/roles/permisos, partes universales, registros
validados por JSON Schema y extensiones configurables, y expone la API interna
que consumen el resto de microservicios.

Es uno de los servicios **siempre activos**: no está gateado por capacidad y es
requisito de arranque del resto (todos consultan `identity` para resolver
capacidades, tenants, temas o partes).

- Reescritura Rust con paridad de contrato:
  [ADR-0017](../../docs/adr/0017-identity-service-en-rust.md)
- Negocios por empresa:
  [ADR-0021](../../docs/adr/0021-negocios-por-empresa.md)
- Fidelización por capacidades:
  [ADR-0019](../../docs/adr/0019-fidelizacion-por-capacidades.md)
- Contrato OpenAPI:
  [`docs/openapi/identity-service/`](../../docs/openapi/identity-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `identity-service` (`packages/identity-service`) |
| Stack | `axum` 0.8, `sqlx` 0.8, `jsonwebtoken` 9, `bcrypt`, `lettre`, `jsonschema` |
| Puerto | `3001` (`PORT`) |
| Esquema | `identity` |
| Outbox | `identity.outbox` (`OUTBOX_TABLA`) |
| Servicios que consume | `storage` (medios/avatares), `stores` (validación de tiendas) |
| Servicios que lo consumen | Todos los gateados, `analytics`, `notifications`, `orders`, `finance`, `logistics`, `field`, `scheduling`, `stores`, `commissions`, `intelligence` |
| Expuesto al host | No |
| Entornos | dev, stage, producción |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL; sin ella no arranca |
| `RABBITMQ_URL` | No | — | Reservada; el servicio no abre RabbitMQ hoy |
| `OUTBOX_TABLA` | No | `identity.outbox` | Reservada; no se escribe outbox hoy |
| `PORT` | No | `3001` | Puerto HTTP |
| `SERVICIO` | No | `identity-service` | Etiqueta de logs/métricas |
| `NODE_ENV` | No | — | `production` exige secretos |
| `JWT_SECRET` | **Sí en producción** | `dev_secret` | Firma HS256 |
| `JWT_ACCESS_TTL` | No | `900s` | Acepta `s`, `m`, `h`, `d`; valor inválido cae a 7 d por compatibilidad |
| `JWT_REFRESH_TTL` | No | `7d` | Idem |
| `ADMIN_EMAIL` | **Sí en producción** | admin de desarrollo | Seed idempotente |
| `ADMIN_PASSWORD` | **Sí en producción** | admin de desarrollo | Seed idempotente |
| `AUTH_EXIGIR_CORREO_VERIFICADO` | No | `true` en producción, `false` en dev | Exige correo verificado; el registro no entrega tokens |
| `MFA_ENCRYPTION_KEY` | Prod | — | Base64 de 32 bytes para AES-256-GCM del secreto TOTP |
| `AUDITORIA_RETENCION_DIAS` | No | `365` | Días de retención de auditoría (`0` = conservar) |
| `TENANT_PURGE_SERVICES` | No | `storage=<url>` | Servicios para purga y exportación tenant |
| `EXPORT_SIGNING_KEY` | No | `JWT_SECRET` | Clave HMAC-SHA256 para firmar exportaciones |
| `AUDITORIA_RETENCION_MODO` | No | `anonimizar` | `anonimizar` conserva el rastro; `eliminar` borra |
| `FRONTEND_URL` | No | `http://localhost:5173` | Enlaces de invitación/reset/verificación |
| `STORAGE_SERVICE_URL` | No | `http://storage-service:3010` | Validar medios |
| `STORES_SERVICE_URL` | No | `http://stores-service:3003` | Validar tiendas |
| `INTERNAL_API_KEY` | Recomendada | — | Protege `/internal/**` y llamadas salientes |
| `PLATFORM_TENANT_ID` | No | — (sin default) | Tenant de assets de plataforma; sin él, la configuración de sitio no puede resolver medios de plataforma (`PLATFORM_TENANT_ID_NO_CONFIGURADO`) |
| `PLATFORM_LOGISTICS_TENANT_ID` | No | `00000000-0000-4000-8000-000000000001` (aplicado por el seed) | Flota global |
| `EMAIL_MODE` | No | `log` | `log` o `smtp` |
| `EMAIL_FROM` | Sí con `smtp` | — | Remitente |
| `SMTP_HOST` | Sí con `smtp` | — | Servidor SMTP |
| `SMTP_PORT` | No | `587` | Puerto |
| `SMTP_TLS_ENABLED` | No | `true` | Exige TLS/STARTTLS; `false` sólo para Mailpit local en desarrollo y sin credenciales |
| `SMTP_USER` / `SMTP_PASSWORD` | Juntos | — | Autenticación SMTP |
| `PLATAFORMA_PLAN_EMPRESARIO` | No | `empresario_plus` | Plan que habilita más de un negocio (ADR-0046) |
| `PLATAFORMA_EMPRESAS_GRATIS` | No | `1` | Empresas incluidas sin cobro |
| `PLATAFORMA_NEGOCIOS_GRATIS` | No | `1` | Negocios incluidos sin cobro |

`EMAIL_MODE` distinto de `log`/`smtp` impide arrancar. Con `smtp`,
`SMTP_HOST`, `EMAIL_FROM` y el par usuario/contraseña se validan al inicio.
`SMTP_TLS_ENABLED=false` sólo es válido para `SMTP_HOST=mailpit`,
`SMTP_PORT=1025` y sin credenciales SMTP; los demás destinos requieren TLS.
El E2E lo usa sólo con el Mailpit efímero que no anuncia STARTTLS.

---

## 2. Arquitectura interna

```
src/
├── main.rs              bootstrap: config, pool, dependencias, seed, router
├── auth.rs              JWT HS256, bcrypt, claims, UsuarioContexto
├── config.rs            entorno + fail-fast
├── db.rs                pool, transacciones, RLS, helpers ISO/UUID
├── email.rs             puerto Email: LogEmail / SmtpEmail (lettre)
├── seed.rs              admin inicial + tenant de plataforma (best-effort)
├── storage.rs           cliente interno de storage (validar avatar/medio)
├── stores.rs            cliente interno de stores (validar tienda)
├── http/                router, handlers, envelope, validación
└── dominio/
    ├── api_keys.rs             API keys hasheadas + canje por JWT
    ├── auditoria.rs            bitácora por empresa/negocio
    ├── auth.rs                 registro, login, refresh, logout
    ├── capacidades.rs          capacidades por empresa
    ├── configuracion_operativa.rs  modelos/items/fulfillment efectivos
    ├── configuracion_sitio.rs  branding de plataforma
    ├── empresas.rs             alta, perfil, slug, pack, vendedores,
    │                            estado, exportación y eliminación del tenant
    ├── extensiones.rs          JSON Schema versionado por empresa
    ├── features.rs             mapa capacidad → features del manifiesto
    ├── media.rs                fondos imagen/video + posiciones
    ├── negocios.rs             unidad operativa (vertical/pack propios)
    ├── packs.rs                verticales, packs, modelos
    ├── partes.rs               personas/organizaciones y relaciones
    ├── password_reset.rs       token de un uso, 30 min, hash SHA-256
    ├── personal.rs             roles, puestos, asignaciones, invitaciones
    ├── regiones.rs             regiones tributarias por pack y tenant
    ├── registros.rs            historia clínica, recetas, expedientes
    ├── sesiones.rs             refresh persistido y revocable
    ├── slug.rs                 normalización, reservados, UUID
    ├── tema.rs                 plantillas, colores, herencia
    └── usuarios.rs             credenciales, perfiles, avatares
```

**Regla de capas:** `http/` valida y traduce; `dominio/` contiene reglas y SQL
explícito. `AppState` inyecta `Arc` de cada servicio de dominio; los clientes
internos son objetos concretos (`Storage`, `Stores`).

---

## 3. Autenticación y sesiones

### Registro y login

- `POST /api/v2/auth/registro` acepta únicamente roles **`empresario`** y
  **`comprador`** (`ROLES_REGISTRABLES`). Cualquier otro rol exige
  `POST /auth/crear-usuario` por un admin.
- Contraseñas con **bcrypt coste 10**; nunca se devuelve el hash.
- Login compara con `bcrypt::verify`; error genérico para no filtrar si el
  correo existe.
- Política de contraseñas: 12–128 caracteres, mayúscula, minúscula, número,
  símbolo, sin correo/nombre y sin contraseñas comunes.
- Lockout por cuenta: 5 intentos fallidos bloquean 15 minutos (`423
  CUENTA_BLOQUEADA`); el éxito resetea el contador.
- Login, refresh, cambio de negocio y aceptación de invitación exigen que la
  empresa del usuario esté `activa`; si está `suspendida` o `inactiva`
  responden `403`.
- Verificación de correo: `POST /auth/verificar-correo` y
  `POST /auth/reenviar-verificacion`. Con `AUTH_EXIGIR_CORREO_VERIFICADO=true`
  (default en producción) login/refresh exigen `correo_verificado_en`.
- MFA opcional TOTP: `POST /auth/mfa/iniciar`, `/confirmar` y `/desactivar`.
  El login acepta `codigo_mfa` (TOTP o código de recuperación). Los códigos de
  recuperación se muestran una vez y se guardan hasheados.
- Sesiones/dispositivos: `GET /auth/sesiones` y
  `DELETE /auth/sesiones/{id}`; se guarda `user_agent`, `ip` (desde
  `X-Client-IP` del gateway) y último uso.
- El secreto TOTP se guarda cifrado con AES-256-GCM (`v1:...`) usando
  `MFA_ENCRYPTION_KEY`; los secretos legacy se migran al confirmar.
- Teléfono de contacto para notificaciones: `PATCH /api/v2/usuarios/{id}/telefono`.
- Reset administrativo: `POST /api/v2/usuarios/{id}/mfa/reset`
  (`super_admin`/`admin`) limpia el MFA y revoca sesiones.
- `POST /auth/refresh` valida firma, `tipo = refresh`, `jti` y que la sesión
  siga activa en `identity.sesiones`. Rota el refresh y devuelve un par nuevo.
- `POST /auth/logout` marca `revocada_en`; el refresh deja de servir.
- `GET /auth/me` devuelve el perfil de la sesión actual.
- `POST /auth/negocio-activo` re-emite tokens con otro `negocio_id` si el
  usuario tiene acceso; `GET /auth/negocios` lista los disponibles.

### Claims del JWT

| Claim | Contenido |
|---|---|
| `sub` | UUID del usuario |
| `email` | Correo |
| `rol` | Rol global (9 posibles) |
| `tipo` | `access` o `refresh` |
| `nombre` | Nombre visible |
| `tenant_id` | Empresa activa |
| `personal_id` | Ficha de personal |
| `participacion_porcentaje` | Participación del vendedor |
| `rol_empresa_id` | Rol configurable dentro de la empresa |
| `negocio_id` | Negocio activo |
| `permisos` | Lista de permisos JSON (`ordenes.leer`, …) |
| `jti` | Solo refresh; permite revocar |
| `iat`, `exp` | Emisión y expiración |

`expira_en` en la respuesta es el **TTL en segundos** (900), no un timestamp.

### Contraseñas y recuperación

- `POST /auth/cambiar-contrasena` (autenticado) exige la contraseña actual.
- `POST /auth/restablecer-contrasena` responde **siempre igual** exista o no el
  correo (OWASP A07).
- `POST /auth/restablecer-contrasena/confirmar` consume un token de un solo uso:
  en BD solo se guarda su SHA-256, vence a los **30 minutos**, y solicitar uno
  nuevo invalida los anteriores.
- La entrega usa `Email`: en dev escribe el enlace en logs; con SMTP usa
  `lettre` y `FRONTEND_URL` para construir el enlace.

### API keys (máquina a máquina)

- Se crean con `POST /api/v2/api-keys`; el secreto se muestra **una vez**.
- Se guarda solo el hash SHA-256 y un `prefijo` para identificarla.
- `POST /api/v2/auth/token` intercambia `{api_key}` por un **JWT de acceso
  corto** sin refresh. Roles permitidos: `empresario`, `vendedor`, `logistica`,
  `coordinador`, `supervisor`, `operativo`.
- Revocación con `DELETE /api/v2/api-keys/{id}`.
- La key nunca se envía a los microservicios ni se acepta desde un navegador
  (R-GW-8).

---

## 4. Tenancy: empresa, negocio, pack y capacidades

### Empresa

- `POST /api/v2/empresas` crea la empresa y el negocio **`principal`** en la
  misma operación, con `vertical_id`, `pack_codigo`/`pack_version` y
  `region_tributaria` (si se omite, la predeterminada del pack).
- Acepta `capacidades_extra` para add-ons (p. ej. `fidelizacion`).
- `GET /regiones-tributarias?pack_codigo=&pack_version=` lista las regiones que
  el pack admite (selector del onboarding).
- `GET/PUT /empresas/{id}/region-tributaria` consulta o cambia la región con
  validación contra el pack, historial y auditoría. El manifiesto de features
  expone `region_tributaria` y sus features `fiscal.*`.
- `GET/PUT /packs/{codigo}/{version}/regiones` lista y reemplaza las regiones
  declaradas por un pack; el PUT es solo `super_admin`/`admin` y exige una
  predeterminada.
- `GET /empresas/{id}` resuelve por UUID **o slug**; solo miembros o admins.
- `PATCH /empresas/{id}/perfil` actualiza datos comerciales/fiscales y logo.
- `PATCH /empresas/{id}/slug` crea un slug nuevo, guarda historial en
  `identity.empresa_slugs` y los anteriores responden `301`.
- `PATCH /empresas/{id}/pack` cambia/actualiza el pack versionado.
- `PATCH /empresas/{id}/estado` (solo `super_admin`/`admin`) cambia el tenant
  entre `activa`, `suspendida` e `inactiva`. Suspender/cerrar revoca las
  sesiones de todos sus usuarios; el login, refresh y las API keys se rechazan
  mientras no esté `activa`. La reactivación queda para un administrador.
- `POST /empresas/{id}/exportacion` crea un job asíncrono que recolecta identity
  y hace fan-out a `TENANT_PURGE_SERVICES`; el archivo
  `core-engine.tenant-export.v2` se firma con HMAC-SHA256.
- `GET /empresas/{id}/exportacion/{job}` consulta el estado y
  `GET /exportaciones/{job}/descarga` baja el archivo firmado.
- `GET /empresas/{id}/exportacion` mantiene la exportación síncrona de identity
  (empresa, asignaciones, usuarios sin hash, negocios, partes, contactos,
  direcciones, relaciones, capacidades, extensiones, slugs y auditoría). El
  campo `alcance.pendiente` lista los servicios cuyos datos aún no se incluyen.
- `POST /empresas/{id}/eliminacion` cierra el tenant y crea una solicitud con
  retención (`retencion_dias`, default 30). `GET` consulta el estado y
  `DELETE` cancela la solicitud antes de que venza.
- `POST /empresas/{id}/eliminacion/purgar` (solo `super_admin`) funciona cuando
  venció la retención: anonimiza la identidad del tenant, conserva filas ancla
  para no romper referencias y marca la solicitud como `purgada`.
- `POST /empresas/{id}/vendedores` crea un vendedor con participación
  predeterminada (regla RN-04).

### Negocio

- Es la unidad operativa con **vertical y pack propios** (ADR-0021).
- Una empresa simple tiene `principal`; añadir un segundo negocio exige un
  `empresario` activo y pasa por `POST /empresas/{id}/negocios`.
- Ciclo de vida: `activo`, `suspendido`, `archivado`
  (`PATCH .../negocios/{negocioId}/estado`).
- `POST .../negocios/{negocioId}/plantillas` aplica plantillas de roles por
  vertical; `GET /negocios/plantillas` las lista.
- Transferencia de personal entre negocios:
  `POST /empresas/{id}/personal/{usuarioId}/transferir`.
- Cada negocio tiene sus capacidades (`identity.negocio_capacidades`), que se
  consultan/activan por separado de las de la empresa.

### Suscripción de plataforma (expansión)

- El primer negocio y la primera empresa son gratuitos por defecto; los cupos
  se cambian con `PLATAFORMA_EMPRESAS_GRATIS`/`PLATAFORMA_NEGOCIOS_GRATIS`
  (poner ambos en `0` para cobrar desde el primer empresario).
- Para operar más de `PLATAFORMA_EMPRESAS_GRATIS` empresas o
  `PLATAFORMA_NEGOCIOS_GRATIS` negocios se exige el plan `empresario_plus`
  (USD 35/mes). Sin plan, la operación devuelve `402 SUSCRIPCION_REQUERIDA`.
- La suscripción es por empresario y el cobro es manual en el MVP:
  `POST /api/v2/plataforma/suscripciones` crea la solicitud `pendiente` y un
  admin la confirma con
  `POST /api/v2/admin/plataforma/suscripciones/{id}/confirmar`.
- Consulta: `GET /api/v2/plataforma/suscripciones/me` y
  `GET /api/v2/plataforma/planes`. Los tenants existentes reciben 30 días de
  gracia (`origen='legacy'`).
- Decisión y límites: [ADR-0046](../../docs/adr/0046-suscripcion-plataforma-expansion.md).

### Onboarding idempotente

- `POST /api/v2/onboarding` con `Idempotency-Key` obligatoria crea empresa +
  negocio principal + pack + capacidades + roles en una sola llamada.
- Exige un `empresario` autenticado; no crea usuarios ni admins.
- Devuelve `empresa`, `negocio_id`, `slug`, `capacidades`, `tema` y un
  `access_token` con el tenant nuevo (evita re-loguear).
- Reintento con la misma clave y cuerpo: replay con
  `Idempotency-Replayed: true`; otra clave/cuerpo: `422
  IDEMPOTENCIA_CLAVE_REUTILIZADA`; sin clave: `400`.
- `GET /api/v2/onboarding/{id}` consulta el estado (`en_proceso`, `completado`,
  `fallido`). Tras 2 minutos, un job atascado puede reclamarse y adopta la
  empresa existente por slug.
- `storage` es lazy (bucket al primer uso); dominios personalizados y datos demo
  no están soportados. Decisión: [ADR-0047](../../docs/adr/0047-onboarding-idempotente.md).

### Capacidades y manifiesto

- `GET /empresas/{empresaId}/capacidades` lista las de la empresa.
- `PATCH /empresas/{empresaId}/capacidades/{capacidad}` activa/desactiva.
- `GET/PATCH /empresas/{empresaId}/negocios/{negocioId}/capacidades[/{capacidad}]`
  hace lo propio por negocio.
- `GET /empresas/{empresaId}/features` devuelve el **manifiesto**: la unión de
  features de las capacidades activas según `dominio/features.rs`.
- `GET /empresas/{empresaId}/negocios/{negocioId}/features` devuelve el
  manifiesto del negocio; si no tiene capacidades propias, hereda las de la
  empresa.
- Una capacidad desconocida o no permitida devuelve `CAPACIDAD_INVALIDA` /
  `CAPACIDAD_NO_DISPONIBLE`.

### Packs, verticales y modelos

- `GET /packs` lista packs activos; `POST /packs` crea una plantilla.
- `GET/POST /packs/verticales` administra verticales.
- `GET/POST /packs/modelos-operacion` administra modelos reutilizables.
- El modelo `venta_directa` declara `requiere_vendedor: false` y
  `comision: false`; un pack que lo incluye opera sin marketplace (ADR-0023).
- `GET /packs/{codigo}/{version}` devuelve el pack versionado y sus
  capacidades/modelos.
- Un pack incompatible con la vertical del negocio produce
  `PACK_VERTICAL_INCOMPATIBLE`.

---

## 5. Personal, roles y permisos

- **Roles configurables por empresa** (`identity.roles_empresa`) con un JSON de
  permisos; `POST/GET /empresas/{id}/roles` y
  `PATCH /empresas/{id}/roles/{rolId}`.
- **Puestos operativos** (`puestos_empresa`): `POST/GET /empresas/{id}/puestos`.
- **Asignaciones** (`asignaciones_personal`): un miembro se asigna a un rol y,
  opcionalmente, a una tienda/puesto. `POST/GET
  /empresas/{id}/asignaciones-personal`.
- **Invitaciones** de personal: enlace de **72 h** con token hasheado; se
  revocan y se consumen por
  `GET/POST /api/v2/incorporaciones/{token}[/aceptar]`.
- Aceptar una incorporación crea cuenta, membresía y sesión, y valida la tienda
  contra `stores-service`.
- Permisos frecuentes: `personal.leer`, `personal.gestionar`,
  `capacidades.leer`, `capacidades.gestionar`, `ordenes.leer`,
  `catalogo.leer`, etc. Un rol de negocio puede administrar su negocio aunque
  su rol global no sea `empresario`.
- La autorización fina combina rol global + `rol_empresa_id` + `permisos` +
  `negocio_id`.

---

## 6. Partes universales y registros

### Partes

Una **parte** es una persona u organización del tenant, sin acoplar el modelo a
"cliente" o "paciente":

- `GET/POST /empresas/{id}/partes`
- `GET/PATCH/DELETE /empresas/{id}/partes/{parteId}` (DELETE archiva)
- Contactos: `GET/POST/PATCH/DELETE .../contactos[/{contactoId}]`
- Direcciones: `GET/POST/PATCH/DELETE .../direcciones[/{direccionId}]`
- Relaciones: `GET/POST .../relaciones`, `DELETE .../relaciones/{relacionId}`

Tipos: `persona`/`organizacion`; estados `activo`/`inactivo`/`archivado`.
Contactos: `email`, `telefono`, `whatsapp`, `web`, `otro`. Direcciones:
`fiscal`, `comercial`, `entrega`, `operativa`, `otro`. El borrado es lógico y
arrastra datos derivados (registros, contactos, direcciones) de forma
consistente.

### Registros

- `GET/POST /empresas/{id}/partes/{parteId}/registros`
- `GET/PATCH/DELETE .../registros/{registroId}` (DELETE archiva)
- Esquemas por tipo: `GET/PUT /empresas/{id}/registros/esquemas/{tipo}`,
  `GET .../registros/esquemas`.
- La **capacidad** (`historia_clinica`, `recetas`, `registros`) decide el
  acceso; el rubro no.
- `datos` se valida contra JSON Schema (máx. **64 KiB**) y admite hasta **10
  adjuntos** referenciados de storage.

### Extensiones

- `GET/POST /empresas/{id}/extensiones`
- `PATCH /empresas/{id}/extensiones/{codigo}` activa/desactiva una versión
- `POST .../extensiones/{codigo}/validar` valida datos sin guardar
- `GET/PUT/DELETE .../extensiones/{codigo}/valores/{recursoId}`

Características: versionado por código, **una sola versión vigente**, JSON
Schema sin `$ref`, máximo 64 KiB, activación explícita y valores por recurso.
Un esquema inválido devuelve `ESQUEMA_EXTENSION_INVALIDO`; datos inválidos,
`FORMULARIO_EXTENSION_INVALIDO` / `REGLAS_EXTENSION_INVALIDAS`.

---

## 7. API pública (resumen)

> Fuente de verdad: `docs/openapi/identity-service/public.openapi.json`.
> Todas las rutas pasan por el gateway (`/api/v2/...`).

| Área | Rutas |
|---|---|
| Auth | `POST /auth/{registro,login,refresh,logout,crear-usuario,cambiar-contrasena,restablecer-contrasena,restablecer-contrasena/confirmar,verificar-correo,reenviar-verificacion,vincular-personal,token}`, `POST /auth/mfa/{iniciar,confirmar,desactivar}`, `GET/DELETE /auth/sesiones[/{id}]`, `GET /auth/me`, `GET /auth/negocios`, `POST /auth/negocio-activo` |
| Usuarios | `GET /usuarios`, `GET /usuarios/{id}`, `PATCH /usuarios/{id}/avatar` |
| Empresas | `POST /empresas`, `GET /empresas/{id}`, `PATCH /empresas/{id}/{pack,perfil,slug,estado}`, `GET /empresas/{id}/exportacion`, `GET/POST/DELETE /empresas/{id}/eliminacion`, `POST /empresas/{id}/eliminacion/purgar`, `POST /empresas/{id}/vendedores` |
| Capacidades | `GET/PATCH /empresas/{empresaId}/capacidades[/{capacidad}]`, idem por negocio, `GET /empresas/{empresaId}/features`, `GET /empresas/{empresaId}/negocios/{negocioId}/features` |
| Packs | `GET/POST /packs`, `GET/POST /packs/verticales`, `GET/POST /packs/modelos-operacion`, `GET /packs/{codigo}/{version}` |
| Negocios | `GET/POST /empresas/{empresaId}/negocios`, `POST .../negocios/{id}/plantillas`, `PATCH .../negocios/{id}/estado`, `GET /negocios/plantillas`, `POST /empresas/{id}/personal/{usuarioId}/transferir` |
| Personal | `GET/POST /empresas/{id}/roles`, `PATCH .../roles/{rolId}`, `GET/POST /empresas/{id}/puestos`, `GET/POST /empresas/{id}/asignaciones-personal`, `GET/POST /empresas/{id}/invitaciones/personal`, `POST .../invitaciones/personal/{id}/revocar`, `GET/POST /incorporaciones/{token}[/aceptar]` |
| Partes | CRUD de partes, contactos, direcciones y relaciones bajo `/empresas/{id}/partes/**` |
| Registros | CRUD de registros y esquemas bajo `/empresas/{id}/partes/{parteId}/registros` y `/empresas/{id}/registros/esquemas` |
| Extensiones | `/empresas/{id}/extensiones/**` |
| Configuración | `GET/PATCH /configuracion/sitio`; fondos y tema de plataforma |
| Auditoría | `GET /empresas/{empresaId}/auditoria` |
| API keys | `GET/POST /api-keys`, `DELETE /api-keys/{id}`, `POST /auth/token` |

Roles: el gateway restringe por rol global; `identity` aplica además la
autorización por tenant/negocio/permiso. Un empresario solo opera su empresa y
solo si sigue siendo miembro activo con rol `empresario`.

---

## 8. API interna (`x-internal-key`)

| Método | Ruta | Consumidor típico |
|---|---|---|
| GET | `/internal/usuarios/{id}` | Cualquier servicio que hidrata usuarios |
| GET | `/internal/vendedores` | commissions, stores, intelligence |
| GET | `/internal/contar-vendedores` | reportes |
| GET | `/internal/empresas/{id}` | Todos |
| GET | `/internal/empresas/{id}/configuracion` | item, pricing, inventory, orders, logistics |
| GET | `/internal/empresas/{id}/tema` | stores |
| GET | `/internal/configuracion-sitio/tema` | stores |
| POST | `/internal/empresas/capacidades` | Gating por lote de empresas |
| POST | `/internal/extensiones/valores` | Servicios que leen extensiones |
| GET | `/internal/partes/{id}` | orders, finance, field, analytics |

Las respuestas internas no usan envelope. La comparación de
`x-internal-key` es en tiempo constante. Un fallo de red se traduce en
`SERVICIO_NO_DISPONIBLE`; un 404 en `NO_ENCONTRADO`.

---

## 9. Eventos y outbox

**Hoy `identity` no publica ni consume eventos de negocio.** Toda la integración
con otros servicios es **síncrona por API interna** (`/internal/...`), que es
justo el caso permitido por R-AR-2 para consultas de lectura: capacidades,
empresas, tema, partes y configuración operativa.

- El compose define `OUTBOX_TABLA=identity.outbox`, pero el servicio **no tiene
  módulo `outbox.rs`, no abre RabbitMQ y no escribe eventos**. La variable queda
  reservada para una evolución futura.
- Consecuencia práctica: cuando cambia una capacidad o un negocio, los demás
  servicios lo ven en su siguiente consulta a identity; no hay invalidación por
  bus. La caché de tema de `stores` usa TTL (`TEMA_HERENCIA_TTL_MS`) para acotar
  la ventana de datos obsoletos.
- Si en el futuro se publican `empresa.creada`, `capacidad.actualizada`, etc.,
  deberá añadirse el outbox transaccional y documentarse aquí.

---

## 10. Seguridad y aislamiento

- **RLS**: el pool fija `request.jwt.claims`; las consultas filtran por
  `empresa_id` y, cuando existe, `negocio_id`. `99_rls.sql` activa las
  políticas en Supabase.
- **Autorización en dos capas**: rol global (gateway + `auth.rs`) y
  tenant/negocio/permiso (dominio). Los handlers de negocio comprueban
  membresía activa.
- **Borrado lógico**: partes, contactos, direcciones, relaciones y registros se
  archivan; no se borran filas con historia.
- **Tokens**: refresh con `jti` persistido; reset con hash y un solo uso;
  invitaciones con hash y 72 h; API keys con hash.
- **Slugs**: normalización determinista, lista de 37 reservados, rechazo de
  UUID, sufijos ante colisión (máx. 50 intentos) e historial con `301`.
- **Seed**: el admin y el tenant de plataforma se crean de forma idempotente y
  *best-effort*; un fallo se registra y no impide arrancar.

---

## 11. Observabilidad y operación

- `GET /health`: liveness.
- `GET /metrics`: métricas Prometheus comunes
  (`core_engine_http_requests_total`, errores, latencia, outbox).
- `GET /docs`, `/docs-json`: OpenAPI versionado.
- Logs JSON con `x-request-id` y contexto por tarea.
- Apagado ordenado en `SIGINT`/`SIGTERM`; cierra pool y deja de aceptar
  peticiones.
- Worker de outbox cada segundo; sin consumidores de eventos propios.

### Diagnóstico frecuente

| Síntoma | Causa probable |
|---|---|
| `401` en todas las rutas | `JWT_SECRET` distinto entre gateway e identity |
| `403 CAPACIDAD_NO_ACTIVA` en un servicio | Capacidad no activada en empresa/negocio |
| Refresh inválido tras reinicio | Sesión revocada o `jti` no persistido |
| Invitación “no válida” | Token vencido (72 h), ya usado o revocado |
| `PACK_VERTICAL_INCOMPATIBLE` | Pack de otra vertical |
| `NEGOCIO_INCOHERENTE` | Segundo negocio sin empresario activo |

---

## 12. Pruebas

```bash
cd packages/identity-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --summary-only
```

- `tests/http_auth.rs`: registro/login/refresh/logout, permisos y flujo HTTP.
- `tests/auth_pg.rs`: persistencia de sesiones, revocación y RLS contra
  PostgreSQL real (`TEST_DATABASE_URL`).
- Cobertura de referencia del gate Rust: 79/75 (medida 81,65/76,98).
- Casos cubiertos: seed idempotente, slugs con historial, capacidades por
  negocio, invitaciones, extensiones, registros, API keys y selección/cambio de
  región tributaria con historial (ADR-0049).

---

## 13. Referencias

- [ADR-0017 · identity-service en Rust](../../docs/adr/0017-identity-service-en-rust.md)
- [ADR-0021 · negocios por empresa](../../docs/adr/0021-negocios-por-empresa.md)
- [ADR-0049 · regiones tributarias](../../docs/adr/0049-regiones-tributarias.md)
- [ADR-0005 · autenticación de gateway sin gate de borde](../../docs/adr/0005-autenticacion-gateway-sin-gate-de-borde.md)
- [Runbook multi-vertical](../../docs/runbook-multi-vertical.md)
- [Runbook de autenticación del gateway](../../docs/runbook-gateway-auth.md)
- [Contrato público](../../docs/openapi/identity-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/identity-service/internal.openapi.json)
- [README de paquetes](../README.md)
