# Core Engine · storage-service (Rust)
# Contexto de build: raíz del repositorio.
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/storage-service/Cargo.toml packages/storage-service/Cargo.lock ./packages/storage-service/
COPY docs/openapi/storage-service ./docs/openapi/storage-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/storage-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/storage-service/src ./packages/storage-service/src
COPY packages/storage-service/tests ./packages/storage-service/tests
RUN touch packages/storage-service/src/main.rs \
    && cd packages/storage-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/storage-service/target/release/storage-service /usr/local/bin/storage-service
ENV NODE_ENV=production
EXPOSE 3010
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/storage-service"]
