# identity-service · Rust

Reescritura en Rust de `identity-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0017-identity-service-en-rust.md`](../../../docs/adr/0017-identity-service-en-rust.md).

El crate vive en `packages/identity-service/rust/` durante la migración; se
moverá a la raíz del paquete en la conmutación (PR 9). Hasta entonces el
runtime activo sigue siendo el TypeScript.

## Comandos

```bash
cd packages/identity-service/rust
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

## Alcance actual (PR 1)

- Configuración con `fail-fast` de variables obligatorias y TTL de JWT.
- JWT HS256 de `access`/`refresh` con el mismo payload que
  `packages/shared/src/auth/jwt.utils.ts` y `jti` en el refresh.
- bcrypt coste 10 y contexto `x-user-*` del gateway.
- PostgreSQL con claims de RLS (`request.jwt.claims`).
- Errores `{codigo, mensaje, detalles?}`, envelope `{data, meta}`, logs JSON,
  `x-request-id` y métricas Prometheus.
- `/health`, `/metrics`, `/docs` y `/docs-json` (OpenAPI versionado).
- Apagado ordenado ante `SIGINT`/`SIGTERM`.
- Gate Rust en CI con cobertura 84/73 (medida 85,24/74,88).

Pendiente en los PR 2-9: auth/sesiones, usuarios, empresas, personal, partes,
registros, extensiones, API interna, seed, correo de restablecimiento, imagen
Docker y conmutación.

## Cobertura

```bash
cargo llvm-cov --ignore-filename-regex 'src/main\.rs'
```
