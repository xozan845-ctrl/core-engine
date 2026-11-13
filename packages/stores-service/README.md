# stores-service · Rust

Reescritura en Rust de `stores-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0011-stores-service-en-rust.md`](../../docs/adr/0011-stores-service-en-rust.md).

El crate vive en la raíz del paquete y el servicio se construye con `cargo`
(igual que `scheduling-service`, `notifications-service`, `field-service` y
`logistics-service`).

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL (fail-fast, R-MS-5)
```

Pruebas de integración (solo corren con una BD con los esquemas `stores` e
`identity`; el test los provisiona en una BD efímera):

```bash
TEST_DATABASE_URL=postgres://user:pass@localhost:5432/core_engine_test cargo test
```

Variables de entorno: `PORT` (3003), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`,
`STORAGE_SERVICE_URL`, `CATALOG_SERVICE_URL`, `PLATFORM_TENANT_ID`,
`TEMA_HERENCIA_TTL_MS`, `INTERNAL_REQUEST_TIMEOUT_MS`.

## Alcance actual

- Configuración con `fail-fast` de `DATABASE_URL`/`RABBITMQ_URL`.
- `/health` (`{servicio, uptime_seg}`), `/metrics` (Prometheus), `/docs` y
  `/docs-json` (OpenAPI público versionado).
- Envelope `{data, meta}` en rutas públicas, errores `{codigo, mensaje,
  detalles?}`, `x-request-id` y logs JSON con contexto de correlación.
- Tienda del vendedor y de la empresa, resolución por UUID/slug con historial
  y redirección 301.
- Logo, fondos de escritorio y móvil, personalización (plantilla, tema, hero y
  SEO) y contenido del storefront, con límites y validación contra `storage`.
- Herencia visual `plataforma → empresa → tienda` con caché TTL y degradación
  a defaults si identity no responde.
- Ofertas de productos, servicios y alquileres con RN-01/RN-02, histórico de
  precios, proyección pública y sincronización de stock.
- Consumidor `stores.stock` (`stock.updated`) con idempotencia por `event_id`.
- Superficie interna `internal/tiendas/**` y `internal/ofertas/**` con
  `x-internal-key`.
- **Imagen Docker** multi-stage (`rust:1.89-bookworm` → `debian:bookworm-slim`)
  con el mismo contrato de contenedor (puerto 3003).
- **Cobertura** medida con `cargo-llvm-cov` en el job `rust` de CI.
