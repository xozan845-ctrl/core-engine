# Core Engine · identity-service (Rust)
# Contexto de build: raíz del repositorio.
#   docker build -f packages/identity-service/Dockerfile -t core-engine-identity-service .
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/identity-service/Cargo.toml packages/identity-service/Cargo.lock ./packages/identity-service/
COPY docs/openapi/identity-service ./docs/openapi/identity-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/identity-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/identity-service/src ./packages/identity-service/src
RUN touch packages/identity-service/src/main.rs \
    && cd packages/identity-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/identity-service/target/release/identity-service /usr/local/bin/identity-service
ENV NODE_ENV=production
EXPOSE 3001
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/identity-service"]
