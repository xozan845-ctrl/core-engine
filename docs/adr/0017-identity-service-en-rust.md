# ADR-0017: Reescritura de `identity-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación en curso (PR 1-4: andamiaje, auth, dominio, empresas y personal)
- **Ámbito**: `identity-service` (HTTP público e interno, PostgreSQL/RLS,
  JWT HS256, sesiones, usuarios, empresas, personales, packs, extensiones,
  partes, registros, API keys, correo de restablecimiento, seed y métricas)
- **Referencias**: [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0003](./0003-notificaciones-correo.md),
  [ADR-0004](./0004-scheduling-service-en-rust.md),
  [ADR-0005](./0005-autenticacion-gateway-sin-gate-de-borde.md),
  [ADR-0014](./0014-finance-service-en-rust.md),
  [ADR-0016](./0016-catalog-service-en-rust.md)

## Contexto

`identity-service` es el último servicio NestJS del repositorio y la autoridad
de tenants, usuarios, roles, capacidades y configuración de la plataforma. Es
también el emisor de los JWT HS256 que valida el gateway, y el único servicio
con contraseñas (bcrypt) y tokens de restablecimiento.

Su superficie actual es ~7.200 líneas TypeScript sin contar pruebas:

- **Auth**: registro, login, refresh, logout, `/auth/me`, crear usuario,
  cambiar contraseña, restablecer contraseña (token SHA-256 de un solo uso) y
  vinculación de personal. Emite `access` (900 s) y `refresh` (7 días) con el
  mismo payload que `@core/shared` y persiste el `jti` en `identity.sesiones`.
- **Usuarios y API keys**: perfiles, avatar, listado de usuarios y API keys
  intercambiables por JWT (ADR-0005).
- **Empresas, capacidades, packs y verticales**: alta de empresa, perfil,
  slug, pack, vendedores, capacidades por empresa, verticales, packs,
  modelos de operación y configuración del sitio.
- **Personal**: roles, puestos, asignaciones, invitaciones e incorporaciones
  por token.
- **Partes y registros**: clientes/proveedores, contactos, direcciones,
  relaciones y registros genéricos (historia clínica, recetas, etc.).
- **Extensiones y media**: extensiones por empresa con JSON Schema, fondos de
  plataforma/empresa/tienda y personalización.
- **Interno**: lectura de usuarios, vendedores, empresas, tema y capacidades
  para `scheduling`, `orders`, `stores`, `field`, `catalog` y `finance`.
- **Seed**: super administrador inicial y tenant de flota de plataforma.

No publica eventos de dominio por Outbox; su integración saliente es SMTP
(restablecimiento de contraseña) y `storage-service` (metadatos de medios).
Actualmente usa `EMAIL_PROVIDER` de `@core/shared` (SMTP/Mailpit).

## Decisión

### 1. Invariantes que no cambian

| Área | Invariante |
|---|---|
| Superficie | mismas rutas, métodos y códigos; envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores `{codigo, mensaje, detalles?}` sin `meta` |
| JWT | HS256 con `JWT_SECRET`; `tipo=access`/`tipo=refresh`; `exp`, `iat`, `sub`, `email`, `rol`, `nombre`, `tenant_id`, `personal_id`, `participacion_porcentaje`, `rol_empresa_id`; refresh con `jti`; access 900 s y refresh 7 días por defecto (`JWT_ACCESS_TTL`/`JWT_REFRESH_TTL`) |
| Sesiones | cada emisión crea `identity.sesiones` con el `jti`; refresh solo si la sesión sigue activa; logout revoca una o todas; logout idempotente |
| Contraseñas | bcrypt coste 10; mínimo 8 caracteres; mismo mensaje y códigos (`CONTRASENA_DEBIL`, `CORREO_INVALIDO`, `CONFLICTO`) |
| Restablecimiento | token aleatorio de 32 bytes, solo se guarda SHA-256; 30 minutos; un solo uso; solicitar uno nuevo invalida los anteriores; la respuesta no revela si el correo existe |
| Autorización | mismos roles y cabeceras `x-user-*`; `RolesGuard`/`CapabilityGuard` equivalentes; rutas públicas explícitas; `/internal` con `x-internal-key` |
| Persistencia | mismas tablas, columnas, constraints y consultas; RLS por claims (`request.jwt.claims`) y `tenant_id` |
| Semántica | misma validación (DTOs y JSON Schema), mismos códigos de dominio, mismos timestamps ISO 8601 UTC con milisegundos (R-C-4) |
| API keys | misma generación, hash y canje por JWT corto (ADR-0005) |
| Seed | super admin idempotente y tenant de plataforma; sin columnas retiradas (`modalidad_logistica`, `tipo_empresa`) |
| Observabilidad | `/health` `{servicio, uptime_seg}`, `/metrics` Prometheus, logs JSON con `request_id`, `servicio` y nivel |
| Runtime | puerto 3001 (`PORT`), mismas variables de entorno, fail-fast sin `DATABASE_URL` y, en producción, sin `JWT_SECRET`/`ADMIN_*` |

### 2. Implementación destino

- Crate `packages/identity-service/rust/`, binario `identity-service`; se mueve
  a la raíz del paquete en la conmutación (como `catalog-service`).
- `axum` + `tokio`, `sqlx` (PostgreSQL), `jsonwebtoken`, `bcrypt`, `sha2`,
  `rand`, `base64`, `serde`, `uuid`, `thiserror`.
- Capas: `config.rs`, `auth.rs` (JWT, bcrypt y contexto), `db.rs` (pool y RLS),
  `errors.rs`, `metrics.rs`, `logs.rs`, `http/` (router, envelope,
  correlación), `state.rs` y, por dominio, `auth/`, `usuarios/`, `empresas/`,
  `capacidades/`, `packs/`, `personal/`, `partes/`, `registros/`,
  `extensiones/`, `configuracion_sitio/`, `api_keys/` y `seed.rs`.
- Estado explícito con `Arc` (equivalente a la DI de NestJS, R-MS-3).
- Sin Outbox mientras el TS no publique eventos: si un dominio necesita
  emitir, se añade en su PR con la misma transacción.
- El correo de restablecimiento se encapsula en un puerto (`Email`) para
  poder usar SMTP (lettre) o la API de `notifications-service` sin cambiar el
  dominio.

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test`.
2. Pruebas de JWT: payload exacto, `access`/`refresh`, `jti`, expiración y
   rechazo de firma/rol.
3. Pruebas de contraseñas: bcrypt, mínimo de longitud y mensajes.
4. Pruebas PostgreSQL por dominio: sesiones, usuarios, empresas, packs,
   personal, partes y registros con RLS por tenant.
5. Pruebas HTTP: envelope, guards, DTOs, 401/403/404/409, rutas internas y
   OpenAPI servido con `x-servicio`.
6. Cobertura Rust en el job `rust` de CI (`cargo-llvm-cov`) con gate medido y
   redondeado a la baja con margen de 1 punto.
7. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | ADR y andamiaje: configuración, logs, errores, métricas, envelope, auth (JWT/bcrypt), DB/RLS, `/health`, `/metrics`, `/docs` y job `rust` de CI | `cargo fmt`, `clippy`, `test` y gate de cobertura verdes |
| 2 | Auth y sesiones: registro, login, refresh, logout, `/auth/me`, crear usuario, cambiar contraseña, restablecer contraseña y vincular personal | Pruebas de JWT, sesiones y contraseñas; PostgreSQL verde |
| 3 | Usuarios (listado/avatar), capacidades, verticales, packs y modelos de operación | Contratos HTTP y RLS por tenant verdes |
| 3b | Empresas, configuración del sitio (tema/media) y API keys | Contratos HTTP y RLS por tenant verdes |
| 4 | Personal: roles, puestos, asignaciones, invitaciones e incorporaciones | Flujo invitación/aceptación y guards verdes |
| 5 | Partes y registros: contactos, direcciones, relaciones y registros genéricos | CRUD y aislamiento por empresa verdes |
| 6 | Extensiones, media/fondos y personalización | JSON Schema y validación de medios verdes |
| 7 | API interna, seed y correo de restablecimiento | Contrato interno y seed idempotente verdes |
| 8 | Pruebas HTTP completas, Dockerfile, compose, scripts, reglas y cobertura | Build y gates verdes; TS sigue activo hasta este PR |
| 9 | Conmutación: retirar TypeScript, `package.json`, tests y baseline Node; añadir a `RUST_SERVICES` | `npm run build`, reglas, OpenAPI y smoke de staging verdes |

## Consecuencias

### Positivas

- El último runtime NestJS desaparece: un solo lenguaje y una sola imagen base
  para todos los servicios.
- `packages/shared` puede retirarse cuando identity deje de ser Node, junto con
  las dependencias npm del backend.
- JWT, sesiones y contraseñas quedan tipados y cubiertos por pruebas Rust.

### Costes y riesgos

- Es el servicio con más superficie de seguridad: cualquier diferencia en el
  payload del JWT o en la revocación de sesiones afecta a todos los servicios.
- `bcryptjs` y `bcrypt` comparten formato, pero la paridad se valida con una
  prueba de hash/verificación y con el login real del smoke.
- La API interna la consumen seis servicios: el contrato debe congelarse con
  pruebas antes de la conmutación.
- El correo de restablecimiento añade una dependencia SMTP; se mantiene detrás
  de un puerto para no acoplar el dominio.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y las
  integraciones PostgreSQL en verde.
- Login, refresh, logout y restablecimiento con el mismo comportamiento y
  códigos que el TS.
- Aislamiento por empresa y rol (RLS) verificado en empresas, personal,
  partes, registros y extensiones.
- OpenAPI, salud, métricas y rutas internas conservan el contrato.
- Smoke de `staging` verde en la primera promoción.

## Estado de implementación (2026-11-13)

- PR 1 completo: crate `packages/identity-service/rust/` con configuración
  fail-fast (JWT, TTL, admin, SMTP y storage), JWT HS256 de access/refresh con
  `jti`, bcrypt coste 10, contexto `x-user-*`, PostgreSQL con claims de RLS,
  errores `{codigo, mensaje, detalles?}`, envelope, logs JSON, métricas
  Prometheus, `/health`, `/metrics`, `/docs` y `/docs-json`.
- PR 2 completo: registro, login, refresh, logout, `/auth/me`, alta
  administrativa, cambio y restablecimiento de contraseña, vinculación de
  personal y revocación de sesiones. Incluye el puerto `Email` con modo `log`
  y SMTP (`lettre`).
- PR 3 completo: listado y perfil de usuarios, avatar validado contra storage,
  capacidades por empresa (con guard de empresario activo), verticales,
  packs, modelos de operación y sus altas administrativas.
- PR 3b completo: empresas (alta, perfil, slug con historial y 301, pack,
  vendedores), configuración del sitio (fondos, panel de IA, plantilla y tema),
  API keys (crear, listar, revocar y canjear por JWT) y la base compartida de
  tema, medios y slug.
- PR 4 completo: roles, puestos, asignaciones, invitaciones e incorporaciones
  de personal, con validación de tiendas contra `stores-service`, tokens de
  72 h de un solo uso y aceptación que crea cuenta, membresía y sesión.
- 114 pruebas (94 unitarias + 6 PostgreSQL de auth/sesiones + 14 HTTP del
  contrato); cobertura medida 81,11 % líneas / 76,07 % funciones (gate 78/75 en
  CI). Las integraciones aplican las migraciones reales de `infra/db/init` en
  orden, no un dump paralelo.
- El TypeScript sigue siendo el runtime activo y el servicio permanece fuera de
  `RUST_SERVICES` hasta el PR 9.
