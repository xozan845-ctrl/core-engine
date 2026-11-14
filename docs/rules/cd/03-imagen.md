# 03 — Higiene de la imagen (R-CD-14)

Reglas de la **imagen de runtime**: usuario no-root y poda de las dependencias de
desarrollo. Complementa el contrato de imágenes de
[`01-despliegue.md`](./01-despliegue.md) (`R-CD-1..8`).

| ID | Regla |
|---|---|
| R-CD-14 | **Higiene de la imagen de runtime**: cada imagen Rust es multi-stage (`rust:<version>-bookworm` → `debian:bookworm-slim`), arranca como usuario no-root (`USER 10001:10001`) y el runtime no lleva toolchain de compilación ni artefactos Node (`node_modules`, `dist`). La toolchain de build debe soportar el `rust-version` de todas las dependencias del `Cargo.lock`. Aplica a `packages/Dockerfile.template` y a cada `packages/<servicio>/Dockerfile`. |
