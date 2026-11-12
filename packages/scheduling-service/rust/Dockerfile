# Core Engine · scheduling-service (Rust)
# Contexto de build: raíz del repositorio (igual que el Dockerfile TypeScript).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/scheduling-service/rust/Cargo.toml packages/scheduling-service/rust/Cargo.lock ./packages/scheduling-service/rust/
COPY docs/openapi/scheduling-service ./docs/openapi/scheduling-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/scheduling-service/rust \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/scheduling-service/rust/src ./packages/scheduling-service/rust/src
COPY packages/scheduling-service/rust/tests ./packages/scheduling-service/rust/tests
RUN touch packages/scheduling-service/rust/src/main.rs \
    && cd packages/scheduling-service/rust \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/scheduling-service/rust/target/release/scheduling-service /usr/local/bin/scheduling-service
ENV NODE_ENV=production
EXPOSE 3011
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/scheduling-service"]
