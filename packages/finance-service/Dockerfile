# Core Engine · finance-service (Rust)
# Contexto de build: raíz del repositorio (igual que el resto de servicios).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/finance-service/Cargo.toml packages/finance-service/Cargo.lock ./packages/finance-service/
COPY docs/openapi/finance-service ./docs/openapi/finance-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/finance-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/finance-service/src ./packages/finance-service/src
RUN touch packages/finance-service/src/main.rs \
    && cd packages/finance-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/finance-service/target/release/finance-service /usr/local/bin/finance-service
ENV NODE_ENV=production
EXPOSE 3007
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/finance-service"]
