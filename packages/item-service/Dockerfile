# Core Engine · item-service (Rust)
# Contexto de build: raíz del repositorio (igual que el resto de servicios).
FROM rust:1.94.1-bookworm AS build
WORKDIR /repo

COPY packages/item-service/Cargo.toml packages/item-service/Cargo.lock ./packages/item-service/
COPY docs/openapi/item-service ./docs/openapi/item-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/item-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/item-service/src ./packages/item-service/src
COPY packages/item-service/tests ./packages/item-service/tests
RUN touch packages/item-service/src/main.rs \
    && cd packages/item-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get upgrade -y \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/item-service/target/release/item-service /usr/local/bin/item-service
ENV NODE_ENV=production
EXPOSE 3015
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/item-service"]
