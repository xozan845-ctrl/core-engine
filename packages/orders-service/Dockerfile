# Core Engine · orders-service (Rust)
# Contexto de build: raíz del repositorio (igual que el resto de servicios).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/orders-service/Cargo.toml packages/orders-service/Cargo.lock ./packages/orders-service/
COPY docs/openapi/orders-service ./docs/openapi/orders-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/orders-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/orders-service/src ./packages/orders-service/src
COPY packages/orders-service/tests ./packages/orders-service/tests
RUN touch packages/orders-service/src/main.rs \
    && cd packages/orders-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/orders-service/target/release/orders-service /usr/local/bin/orders-service
ENV NODE_ENV=production
EXPOSE 3004
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/orders-service"]
