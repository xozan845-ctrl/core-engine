# Core Engine · commissions-service (Rust)
# Contexto de build: raíz del repositorio (igual que el Dockerfile TypeScript).
FROM rust:1.89-bookworm AS build
WORKDIR /repo

COPY packages/commissions-service/Cargo.toml packages/commissions-service/Cargo.lock ./packages/commissions-service/
COPY docs/openapi/commissions-service ./docs/openapi/commissions-service

# Cachea las dependencias con un binario vacío antes de copiar el código real.
RUN cd packages/commissions-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked

COPY packages/commissions-service/src ./packages/commissions-service/src
COPY packages/commissions-service/tests ./packages/commissions-service/tests
RUN touch packages/commissions-service/src/main.rs \
    && cd packages/commissions-service \
    && cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/commissions-service/target/release/commissions-service /usr/local/bin/commissions-service
ENV NODE_ENV=production
EXPOSE 3006
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/commissions-service"]
