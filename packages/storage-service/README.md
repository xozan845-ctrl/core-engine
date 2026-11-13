# storage-service (Rust)

Reescritura en Rust del servicio de almacenamiento de objetos (ADR-0012),
siguiendo el patrón de `scheduling-service`, `notifications-service` y
`field-service`.

- `axum` para HTTP, `sqlx` para PostgreSQL y `aws-sdk-s3` contra MinIO/S3.
- Buckets por tenant (`t-<uuid>`), cuotas, metadata de objetos y URLs
  prefirmadas de subida/descarga.
- Servido público de media (imagen/video) con `Range` y `HEAD` (ADR-28).
- Outbox `storage.outbox` con poller RabbitMQ.
- El contrato HTTP se conserva; el OpenAPI vive en
  `docs/openapi/storage-service/`.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
TEST_DATABASE_URL=... cargo test --test storage_pg
```

El binario Rust es el runtime activo; el TypeScript se retiró en la
conmutación (ADR-0012).
