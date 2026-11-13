# Core Engine · market-intelligence-service (Rust)
# Contexto de build: raíz del repositorio.
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/market-intelligence-service/Cargo.toml packages/market-intelligence-service/Cargo.lock ./packages/market-intelligence-service/
COPY docs/openapi/market-intelligence-service ./docs/openapi/market-intelligence-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/market-intelligence-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/market-intelligence-service/src ./packages/market-intelligence-service/src
COPY packages/market-intelligence-service/tests ./packages/market-intelligence-service/tests
RUN touch packages/market-intelligence-service/src/main.rs \
    && cd packages/market-intelligence-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/market-intelligence-service/target/release/market-intelligence-service /usr/local/bin/market-intelligence-service
ENV NODE_ENV=production
EXPOSE 3009
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/market-intelligence-service"]
