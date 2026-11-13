# Core Engine · field-service (Rust)
# Contexto de build: raíz del repositorio (igual que el Dockerfile TypeScript).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/field-service/Cargo.toml packages/field-service/Cargo.lock ./packages/field-service/
COPY docs/openapi/field-service ./docs/openapi/field-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/field-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/field-service/src ./packages/field-service/src
COPY packages/field-service/tests ./packages/field-service/tests
RUN touch packages/field-service/src/main.rs \
    && cd packages/field-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/field-service/target/release/field-service /usr/local/bin/field-service
ENV NODE_ENV=production
EXPOSE 3008
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/field-service"]
