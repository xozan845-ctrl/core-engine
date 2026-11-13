# ADR-0015: Reescritura del `api-gateway` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado (en curso) — crate Rust con paridad; el TypeScript sigue
  activo hasta el PR de conmutación
- **Ámbito**: `api-gateway` (borde HTTP, JWT HS256, políticas por ruta/rol,
  proxy, rate limit, CORS, diagnóstico, readiness y métricas)
- **Referencias**: [ADR-0004](./0004-scheduling-service-en-rust.md),
  [ADR-0005](./0005-autenticacion-gateway-sin-gate-de-borde.md),
  [ADR-0010](./0010-logistics-service-en-rust.md),
  [ADR-0011](./0011-stores-service-en-rust.md),
  [ADR-0012](./0012-storage-service-en-rust.md)

## Contexto

`api-gateway` era el único servicio NestJS que quedaba en el borde tras las
migraciones de `scheduling`, `notifications`, `commissions`, `field`,
`market-intelligence`, `logistics`, `stores`, `storage`, `orders` y `finance`.
Su lógica real es pequeña (~1.2k líneas sin el OpenAPI generado), pero es el
punto de entrada de todo el tráfico externo y concentra decisiones de
seguridad:

- Valida JWT HS256 (compatible con Supabase Auth) y rechaza refresh tokens en
  rutas de acceso.
- Aplica la tabla de políticas por ruta/rol (Tabla 21) con deny-by-default
  sobre `/api/v2/*`.
- Inyecta la identidad (`x-user-*`, `x-tenant`, `x-user-personal`,
  `x-user-participacion`, `x-user-rol-empresa`) y no reenvía cabeceras internas
  del cliente (R-GW-3 / R-DS-5).
- Enruta a once destinos por patrón y método, con timeout de 30 s y streaming
  binario para media pública.
- Aplica CORS por whitelist, cabeceras de hardening, rate limit por IP
  (300/min global, 10/min login y `/ready`) y correlación `x-request-id`.
- Cierra `/ready`, `/docs`, `/docs-json`, `/docs-yaml` y `/metrics` con
  `x-internal-key` salvo que el entorno los declare públicos (ADR-0005).
- Agrega readiness de los servicios desplegados por entorno (R-CD-12) y expone
  `/health` y `/metrics`.

El contrato HTTP agregado vive en `docs/api/core-engine.openapi.json` y no
cambia con esta decisión.

## Decisión

### 1. Invariantes que no cambian

| Área | Invariante |
|---|---|
| Superficie | mismas rutas/métodos/estados; errores `{codigo, mensaje, detalles?}`; `x-request-id` en todas las respuestas |
| Autenticación | HS256 con `JWT_SECRET`; `tipo=access`; 401 sin token o token inválido; 403 por rol insuficiente |
| Políticas | misma tabla y orden de primera coincidencia; deny-by-default en `/api/v2/`; rutas públicas explícitas |
| Identidad | mismas cabeceras inyectadas; el cliente no puede colar `x-user-*`, `x-tenant`, `x-user-personal`, `x-user-participacion`, `x-user-rol-empresa` ni `x-internal-key` |
| Enrutado | mismos destinos, patrones y métodos (Tabla 21); `logistics` antes del catch-all de `orders` |
| Proxy | query conservada, redirects manuales (301 de slug con `Location`), timeout configurable, 504 `DESTINO_TIMEOUT` y 502 `DESTINO_NO_DISPONIBLE` |
| Media pública | solo `GET`/`HEAD` sobre `/api/v2/storage/public/objetos/<uuid>`; `Range` y streaming sin decodificar |
| Borde | CORS por whitelist sin credenciales; cabeceras de hardening A05; rate limit por IP y cubo |
| Diagnóstico | `/health` público; `/ready`, `/docs*` y `/metrics` con clave interna salvo `READY_PUBLIC`/`DIAGNOSTICOS_PUBLICOS` |
| Readiness | lista por `ENTORNO` (`stage`/`staging` añade `commissions`); `200` solo si todos responden `ok`; caché `READY_CACHE_MS` |
| Runtime | puerto 3000; mismas variables de entorno; fail-fast sin `JWT_SECRET` en producción |

### 2. Implementación destino

- Crate `packages/api-gateway/rust/` con binario `api-gateway`.
- `axum` + `tokio`, `reqwest` (rustls, streaming), `jsonwebtoken`, `regex`,
  `serde`, `uuid`, `thiserror`.
- Capas: `config.rs` (entorno), `auth.rs` (JWT y contexto), `policy.rs`
  (tabla de acceso), `proxy.rs` (destinos y reenvío), `rate_limit.rs`,
  `diagnostics.rs`, `readiness.rs`, `metrics.rs`, `state.rs` y `http/mod.rs`
  (router y middleware de borde).
- Estado explícito con `Arc` (equivalente a la DI de NestJS, R-MS-3).
- `/docs` y `/docs-json` sirven el OpenAPI agregado versionado; `/docs-yaml`
  responde el mismo documento con `content-type` YAML (YAML 1.2 es superconjunto
  de JSON) para conservar la ruta protegida de ADR-0005.

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
2. Pruebas de políticas: rutas públicas, deny-by-default, roles por método,
   refresh rechazado y propagación de contexto.
3. Pruebas de borde: `/health`, cierre de `/docs` y `/ready` sin clave,
   rate limit y enrutado por servicio/método.
4. `cargo llvm-cov` con gate de líneas y funciones en la matriz Rust de CI.
5. El OpenAPI versionado no cambia; el binario sirve el mismo documento.
6. Smoke de `staging` en la primera promoción de la imagen.

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Crate Rust completo en `rust/`, ADR, CI y pruebas | `cargo` verde y gate de cobertura; TS sigue activo |
| 2 | Conmutación: retirar TS, Dockerfile, compose, scripts, reglas y cobertura | Imagen Rust y CI sin `api-gateway` Node |
| 3 | Promoción `develop → staging` | Árbol idéntico y smoke de staging |

## Consecuencias

- El borde completo pasa a un único runtime Rust, con el mismo binario base que
  el resto de servicios y sin toolchain Node en la imagen del gateway.
- El rate limit sigue siendo en memoria y por réplica; una flota horizontal
  comparte cubos solo si el balanceo es sticky. Es el mismo comportamiento que
  el gateway NestJS y se mantiene como deuda explícita.
- La lista de destinos y políticas se replica en `policy.rs`/`proxy.rs`; los
  cambios de Tabla 21 deben tocar ambos archivos y sus pruebas.
- El OpenAPI agregado sigue siendo la fuente del contrato servido; el gateway
  no lo regenera en runtime.
