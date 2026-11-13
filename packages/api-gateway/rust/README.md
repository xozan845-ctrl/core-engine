# api-gateway (Rust)

Reescritura en Rust del API Gateway de Core Engine (ADR-0015), siguiendo el
patrón de los servicios ya migrados (`storage-service`, `orders-service`,
`finance-service`, …).

- `axum` para HTTP, `reqwest` para el proxy con streaming y `jsonwebtoken`
  para HS256.
- Valida JWT de acceso, aplica la tabla de políticas por ruta/rol con
  deny-by-default e inyecta el contexto `x-user-*` / `x-tenant`.
- Enruta a los microservicios por patrón y método, con timeout configurable,
  redirects manuales y media pública con `Range`.
- CORS por whitelist, cabeceras de hardening, rate limit por IP y correlación
  `x-request-id`.
- `/health`, `/ready`, `/docs`, `/docs-json`, `/docs-yaml` y `/metrics`; el
  contrato agregado se sirve desde `docs/api/core-engine.openapi.json`.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --summary-only
```

Durante la migración el TypeScript sigue siendo el runtime activo; el binario
Rust pasa a serlo en el PR de conmutación (ADR-0015).
