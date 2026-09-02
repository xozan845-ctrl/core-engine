# AGENTS.md — Core Engine

## Architecture

- NestJS microservices monorepo (npm workspaces). 9 packages under `packages/`: `shared` (common lib), `api-gateway` (port 8080), and 7 domain services (identity, catalog, stores, orders, logistics, commissions, finance) + `field-service`.
- All traffic routes through the API Gateway (`:8080/api/v1/...`). Services are not exposed directly.
- CQRS + Event Sourcing **only** in `orders-service`. Everything else is plain CRUD.
- RabbitMQ broker (`core-engine.events` topic) + outbox pattern per service (`[schema].outbox` table). DLQ with exponential backoff.
- Postgres 16 with domain-specific schemas (`identity`, `catalog`, `stores`, `orders`, `logistics`, `commissions`, `finance`). Each service connects to its own schema.
- Money is **always** integer centavos internally (`*_cents` columns). API serializes as `"1150.00"` strings (OWASP A02). Use `Money` class from `@core/shared`.

## Commands

```bash
npm install                       # workspaces (requires NTFS — junctions)
npm run build                     # builds shared first, then all 8 services
npm test                          # runs tests: shared, gateway, orders, commissions, finance
npm run demo                      # E2E smoke test (requires docker compose up)
npm run lint --workspaces         # lint all packages (if present)

docker compose up -d --build      # full stack: Postgres, RabbitMQ, all services, Prometheus, Grafana
```

### Single package

```bash
npm run build -w @core/shared
npm run test  -w @core/orders-service
npm run build -w @core/api-gateway
```

### Build order matters

`shared` must build first (other packages depend on it). The root `build` script handles this. If building manually: `npm run build:shared` then the target service.

## Testing

- Jest with `ts-jest`. Test files: `*.spec.ts` in `src/` of each package.
- Unit tests must be isolated (no real DB/RabbitMQ). Use mocks.
- `npm test` runs 45 tests across shared, gateway, orders, commissions, finance.
- `npm run demo` (`scripts/smoke.mjs`) runs the full flow against a live stack.
- Coverage expected to be solid in domain services; PRs must not reduce it.

## Key Conventions

- **Naming**: Files in `kebab-case`, classes/interfaces in `PascalCase`, constants in `UPPER_SNAKE_CASE`. DB tables/columns in `snake_case`.
- **NestJS suffixes**: `*.controller.ts`, `*.service.ts`, `*.module.ts`, `*.command.ts`, `*.handler.ts`, `*.event.ts`, `*.consumer.ts`.
- **Errors**: All errors extend `DomainError` from `@core/shared`. Response format: `{ codigo, mensaje, detalles }`. Use `DomainErrorFilter` (already registered globally).
- **Internal auth**: Service-to-service calls use `x-internal-key` header. Defined in `.env` as `INTERNAL_API_KEY`.
- **Context propagation**: Gateway sets `x-request-id`, `x-tenant`, `x-user-personal` headers. `TrazabilidadInterceptor` + `AsyncLocalStorage` makes `request_id` available in logs.
- **Rate limiting**: Global 300 req/min, login 10/min via `RateLimitMiddleware` in gateway. Returns structured 429.
- **Ports**: Gateway 8080, identity 3001, catalog 3002, stores 3003, orders 3004, logistics 3005, commissions 3006, finance 3007, field 3008.
- **Swagger docs**: Available at `/docs` on the gateway.

## Gateway Routing Gotcha

`PATCH /api/v1/orders/:id/estado` routes to **logistics-service**, not orders-service. This is deliberate — logistics owns state transitions (ADR-03). The orders route is a catch-all that does NOT match this pattern first.

## Database

- Docker: Postgres auto-runs `infra/db/init/01_esquemas.sql` (schemas, tables, seeds).
- Supabase (staging/prod): additionally apply `infra/db/supabase/99_rls.sql` for row-level security.
- Each service uses its own schema. The `OUTBOX_TABLA` env var points to the service's outbox table (e.g., `orders.outbox`).
- Each service's `OUTBOX_TABLA` must be set correctly: `identity.outbox`, `catalog.outbox`, `stores.outbox`, `orders.outbox`, `logistics.outbox`, `commissions.outbox`, `finance.outbox`, `field.outbox`.

## Environment

- Copy `.env.example` to `.env`. Key vars: `DATABASE_URL`, `RABBITMQ_URL`, `JWT_SECRET`, `INTERNAL_API_KEY`, `ADMIN_EMAIL`, `ADMIN_PASSWORD`.
- `.env.example` is **incomplete** for Docker: you also need `RABBITMQ_USER`, `RABBITMQ_PASSWORD`, and `GRAFANA_ADMIN_PASSWORD` (required by `docker-compose.yml`).
- JWT is HS256 (Supabase-compatible). Access TTL 15 min, refresh 7 days.
- All monetary values in the DB use `*_cents` integer columns. The `Money` class handles parsing/formatting.

## Gotchas

- **NTFS required**: npm workspaces create junctions. ExFAT/FAT32 will fail with `EISDIR` on `npm install`.
- **Build before test**: Some packages import from `@core/shared` types. Ensure `shared` is built.
- **field-service has no tests**: It's a newer package (app-test for logistics). No `test` script defined.
- **Outbox polling**: `OutboxService` polls every 1s. Events must be inserted in the same DB transaction as business logic.
- **RLS is manual**: `99_rls.sql` is NOT auto-applied in Docker. Only for Supabase deployments.
- **Two Dockerfiles**: Root `Dockerfile` (used by `docker-compose.yml`) and `packages/Dockerfile.template` (per-service). They use different install strategies (`npm install` vs `npm ci`).
- **DLQ retries**: Max 3 retries with 500ms base exponential backoff before dead-lettering (`REINTENTOS_MAXIMOS=3`, `BACKOFF_BASE_MS=500` in `shared/src/rabbitmq/rabbit.constants.ts`).
- **`.dockerignore` excludes `*.md`**: AGENTS.md and other docs are not copied into Docker images.
