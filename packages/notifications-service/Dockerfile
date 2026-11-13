# Core Engine · notifications-service (Rust)
# Contexto de build: raíz del repositorio (igual que el Dockerfile TypeScript).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/notifications-service/Cargo.toml packages/notifications-service/Cargo.lock ./packages/notifications-service/
COPY docs/openapi/notifications-service ./docs/openapi/notifications-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/notifications-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/notifications-service/src ./packages/notifications-service/src
COPY packages/notifications-service/tests ./packages/notifications-service/tests
RUN touch packages/notifications-service/src/main.rs \
    && cd packages/notifications-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/notifications-service/target/release/notifications-service /usr/local/bin/notifications-service
ENV NODE_ENV=production
EXPOSE 3012
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/notifications-service"]
