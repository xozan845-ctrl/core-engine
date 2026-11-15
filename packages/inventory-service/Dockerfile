# Core Engine · inventory-service (Rust, ADR-0018)
# Contexto de build: raíz del repositorio.
FROM rust:1.94.1-bookworm AS build
WORKDIR /repo

COPY packages/inventory-service/Cargo.toml packages/inventory-service/Cargo.lock ./packages/inventory-service/
COPY docs/openapi/inventory-service ./docs/openapi/inventory-service

RUN cd packages/inventory-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/inventory-service/src ./packages/inventory-service/src
COPY packages/inventory-service/tests ./packages/inventory-service/tests
RUN touch packages/inventory-service/src/main.rs \
    && cd packages/inventory-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/inventory-service/target/release/inventory-service /usr/local/bin/inventory-service
ENV NODE_ENV=production
EXPOSE 3013
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/inventory-service"]
