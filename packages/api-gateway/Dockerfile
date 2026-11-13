# Core Engine · api-gateway (Rust)
# Contexto de build: raíz del repositorio.
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/api-gateway/Cargo.toml packages/api-gateway/Cargo.lock ./packages/api-gateway/
COPY docs/api ./docs/api

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/api-gateway \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/api-gateway/src ./packages/api-gateway/src
RUN touch packages/api-gateway/src/main.rs \
    && cd packages/api-gateway \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/api-gateway/target/release/api-gateway /usr/local/bin/api-gateway
ENV NODE_ENV=production
EXPOSE 8080
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/api-gateway"]
