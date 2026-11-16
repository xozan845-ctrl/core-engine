# Core Engine · pricing-service (Rust, ADR-0018)
FROM rust:1.94.1-bookworm AS build
WORKDIR /repo
COPY packages/pricing-service/Cargo.toml packages/pricing-service/Cargo.lock ./packages/pricing-service/
COPY docs/openapi/pricing-service ./docs/openapi/pricing-service
RUN cd packages/pricing-service \
    && mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked
COPY packages/pricing-service/src ./packages/pricing-service/src
COPY packages/pricing-service/tests ./packages/pricing-service/tests
RUN touch packages/pricing-service/src/main.rs \
    && cd packages/pricing-service \
    && cargo build --release --locked
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /repo/packages/pricing-service/target/release/pricing-service /usr/local/bin/pricing-service
ENV NODE_ENV=production
EXPOSE 3014
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/pricing-service"]
