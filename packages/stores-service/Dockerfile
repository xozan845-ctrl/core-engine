# Core Engine · stores-service (Rust)
# Contexto de build: raíz del repositorio (igual que el resto de servicios).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/stores-service/Cargo.toml packages/stores-service/Cargo.lock ./packages/stores-service/
COPY docs/openapi/stores-service ./docs/openapi/stores-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/stores-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/stores-service/src ./packages/stores-service/src
COPY packages/stores-service/tests ./packages/stores-service/tests
RUN touch packages/stores-service/src/main.rs \
    && cd packages/stores-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/stores-service/target/release/stores-service /usr/local/bin/stores-service
ENV NODE_ENV=production
EXPOSE 3003
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/stores-service"]
