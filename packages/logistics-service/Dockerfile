# Core Engine · logistics-service (Rust)
# Contexto de build: raíz del repositorio (igual que el resto de servicios).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/logistics-service/Cargo.toml packages/logistics-service/Cargo.lock ./packages/logistics-service/
COPY docs/openapi/logistics-service ./docs/openapi/logistics-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/logistics-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/logistics-service/src ./packages/logistics-service/src
COPY packages/logistics-service/tests ./packages/logistics-service/tests
RUN touch packages/logistics-service/src/main.rs \
    && cd packages/logistics-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/logistics-service/target/release/logistics-service /usr/local/bin/logistics-service
ENV NODE_ENV=production
EXPOSE 3005
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/logistics-service"]
