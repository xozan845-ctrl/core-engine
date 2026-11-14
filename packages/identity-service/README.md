# identity-service · Rust

Reescritura en Rust de `identity-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0017-identity-service-en-rust.md`](../../../docs/adr/0017-identity-service-en-rust.md).

El crate vive en la raíz de `packages/identity-service/`; el TypeScript se
retiró en la conmutación (PR 9) y el servicio se construye con `cargo`.

## Comandos

```bash
cd packages/identity-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL (fail-fast, R-MS-5)
```

Variables de entorno: `PORT` (3001), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `OUTBOX_TABLA`, `JWT_SECRET`, `JWT_ACCESS_TTL`,
`JWT_REFRESH_TTL`, `ADMIN_EMAIL`, `ADMIN_PASSWORD`, `FRONTEND_URL`,
`STORAGE_SERVICE_URL`, `INTERNAL_API_KEY`, `NODE_ENV`.

En producción `JWT_SECRET`, `ADMIN_EMAIL` y `ADMIN_PASSWORD` son obligatorias.

## Alcance actual (implementado)

- Configuración con `fail-fast` de variables obligatorias, TTL de JWT y SMTP.
- JWT HS256 de `access`/`refresh` con el mismo payload que
  `packages/shared/src/auth/jwt.utils.ts` y `jti` en el refresh.
- bcrypt coste 10 y contexto `x-user-*` del gateway.
- PostgreSQL con claims de RLS (`request.jwt.claims`).
- **Auth y sesiones**: registro, login, refresh, logout, `/auth/me`, alta
  administrativa, cambio y restablecimiento de contraseña, vinculación de
  personal y revocación de sesiones en `identity.sesiones`.
- **Correo**: puerto `Email` con modo `log` y SMTP (`lettre`) para el enlace de
  restablecimiento.
- **Usuarios**: listado, perfil con control admin/propio, avatar validado
  contra `storage-service`.
- **Capacidades, verticales y packs**: lectura y altas administrativas, con
  guard de empresario activo por tenant.
- **Empresas**: alta con pack/vertical, perfil, slug con historial y 301,
  cambio de pack, logo/fondos y alta de vendedores.
- **Configuración del sitio**: fondos y panel de IA, plantilla y tema.
- **API keys**: creación, listado, revocación y canje por JWT corto.
- **Base compartida**: tema (herencia y overrides), medios (imagen/video) y
  slugs (normalización, reservados y sufijos).
- **Personal**: roles con permisos JSON, puestos, asignaciones, invitaciones
  de 72 h e incorporaciones que crean cuenta, membresía y sesión; tiendas
  validadas contra `stores-service`.
- **Partes y registros**: personas/organizaciones, contactos, direcciones,
  relaciones y registros genéricos con capacidad y adjuntos.
- **Extensiones**: versionado por código, JSON Schema sin `$ref`, activación
  de la versión vigente y valores por recurso validados.
- **API interna**: usuarios, vendedores, empresas, tema, capacidades y partes
  con `x-internal-key`; seed idempotente del admin y del tenant de plataforma.
- Errores `{codigo, mensaje, detalles?}`, envelope `{data, meta}`, logs JSON,
  `x-request-id` y métricas Prometheus.
- `/health`, `/metrics`, `/docs` y `/docs-json` (OpenAPI versionado).
- Apagado ordenado ante `SIGINT`/`SIGTERM`.
- Gate Rust en CI con cobertura 79/75 (medida 81,65/76,98).

El servicio está conmutado: el binario Rust es el runtime activo y el TypeScript fue retirado.

## Imagen Docker

`packages/identity-service/Dockerfile` es la imagen multi-stage activa:

```bash
docker build -f packages/identity-service/Dockerfile -t core-engine-identity-service .
```

Expone el puerto 3001 y ejecuta como usuario `10001:10001`.

## Cobertura

```bash
cargo llvm-cov --ignore-filename-regex 'src/main\.rs'
```
