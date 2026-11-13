# Core Engine · catalog-service (Rust)
# Contexto de build: raíz del repositorio (igual que el resto de servicios).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/catalog-service/Cargo.toml packages/catalog-service/Cargo.lock ./packages/catalog-service/
COPY docs/openapi/catalog-service ./docs/openapi/catalog-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/catalog-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/catalog-service/src ./packages/catalog-service/src
COPY packages/catalog-service/tests ./packages/catalog-service/tests
RUN touch packages/catalog-service/src/main.rs \
    && cd packages/catalog-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/catalog-service/target/release/catalog-service /usr/local/bin/catalog-service
ENV NODE_ENV=production
EXPOSE 3002
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/catalog-service"]
