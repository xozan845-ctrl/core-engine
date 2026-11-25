# 04 — Guía para crear un servicio nuevo

Esta es la **golden path** de Core Engine: la forma soportada de añadir un
microservicio sin romper la arquitectura. Un servicio nuevo debe quedar
desplegable, observable y conforme en **un solo PR** (o una cadena de PRs
apilados con el mismo ADR).

> La suite `services-contract` valida cada paso mecánicamente. Si algo de esta
> guía no está en el manifiesto, no existe para la plataforma.

## 0. Antes de escribir código

### 0.1 ¿De verdad necesitas un servicio nuevo?

Crea un servicio sólo si se cumple **al menos una**:

- tiene un **bounded context** propio con lenguaje y reglas de negocio propias;
- necesita **ciclo de despliegue/escala independiente**;
- posee datos que no deben compartirse con otro contexto;
- tiene un **SLO o perfil de carga** distinto.

No crees un servicio por:

- una entidad o una tabla;
- una pantalla o un endpoint;
- una vertical (farmacia, belleza, flota): eso es configuración/capacidad;
- “limpieza” de un módulo grande sin cambio de contexto.

Si dudas, empieza como módulo del servicio existente y extrae cuando el límite
esté claro (patrón *strangler*).

### 0.2 ADR obligatorio

Crea `docs/adr/00NN-<servicio>-en-rust.md` con:

- contexto y problema de negocio;
- bounded context y capacidades que posee;
- datos que posee y datos que consume;
- eventos que publica/consume;
- perfil (`edge/core/transactional/consumer/olap`) y justificación;
- dependencias síncronas y su degradación;
- SLO y criticidad;
- alternativas descartadas;
- criterios de aceptación.

Actualiza `docs/adr/README.md`. Sin ADR, el servicio no entra al catálogo
(R-AF-5).

## 1. Crear el manifiesto

Crea `packages/<servicio>/service.json`:

```json
{
  "$schema": "../../scripts/config/service-schema.json",
  "apiVersion": "core-engine.io/v1",
  "kind": "Service",
  "metadata": {
    "name": "example-service",
    "boundedContext": "example",
    "description": "Describe el contexto y la capacidad en una frase.",
    "owner": "core-platform",
    "lifecycle": "experimental",
    "tags": ["rust", "core", "example"]
  },
  "spec": {
    "profile": "core",
    "runtime": "rust",
    "port": 3017,
    "schema": "example",
    "outboxTable": "example.outbox",
    "capabilities": ["example_capacidad"],
    "publicApi": true,
    "internalApi": true,
    "environments": ["dev", "stage", "prod"],
    "dependencies": { "sync": ["identity-service"] },
    "events": { "publishes": [], "consumes": [] },
    "slo": { "availability": 99.5, "latencyP95Ms": 1500 },
    "docs": { "readme": "README.md", "openapi": "docs/openapi/example-service" }
  },
  "quality": { "coverage": { "lines": 60, "functions": 60 } }
}
```

**Checklist del manifiesto:**

- [ ] Puerto libre y único (mira `catalogo-servicios.md`).
- [ ] Esquema nuevo, no reutilizado de otro servicio.
- [ ] Perfil correcto; si publica eventos, `transactional` + outbox.
- [ ] Capacidades reales (no inventes capacidades que `identity` no siembra).
- [ ] Dependencias síncronas mínimas y con timeout/degradación.
- [ ] Eventos con nombres de hechos, no comandos.
- [ ] `quality.coverage` con umbrales reales (no 0).

## 2. Crear el crate

```bash
mkdir -p packages/example-service/src/http packages/example-service/tests
cd packages/example-service
cargo init --bin --name example-service .
```

Sigue el esqueleto de [`02-contrato-de-servicio.md`](./02-contrato-de-servicio.md):

```text
src/
├── main.rs        # bootstrap: Config, Db, router, señales
├── lib.rs         # pub mod ...
├── config.rs      # from_env + validar_arranque (única lectura de env)
├── db.rs          # pool, RLS, helpers
├── errors.rs      # ApiError + IntoResponse
├── state.rs       # AppState (Arc)
├── logs.rs        # JSON + request_id
├── metrics.rs     # RED + negocio
├── auth.rs        # headers/capacidades
├── http/
│   ├── mod.rs     # /health, /ready, /metrics, /docs y rutas
│   └── ...
└── ejemplo/       # agregado/casos de uso + SQL explícito
```

**Reglas de código:**

- `main.rs` sólo compone; no contiene SQL ni reglas.
- `config.rs` valida al arrancar y falla con `exit(1)` si falta algo crítico.
- El dominio no depende de `axum` salvo extracción de cabeceras.
- SQL explícito con `sqlx`; sin ORM; sin `format!` con input de usuario.
- Montos en centavos enteros.
- Nada de `unwrap()`/`expect()` en rutas de request.
- Archivos ≤ 800 líneas de producción y ≤ 20 funciones públicas con SQL
  (R-MS-7).

## 3. Datos

1. Añade la migración aditiva en `infra/db/init/NN_<servicio>.sql`:

```sql
CREATE SCHEMA IF NOT EXISTS example;

CREATE TABLE IF NOT EXISTS example.aggregates (
  id uuid PRIMARY KEY,
  empresa_id uuid NOT NULL,
  negocio_id uuid,
  creado_en timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);
```

2. Añade RLS y política en `infra/db/supabase/99_rls.sql`; si la tabla no
   encaja en el barrido dinámico de `99z_rls_hardening.sql`, añade la política
   específica ahí. Toda tabla debe quedar con `FORCE RLS` y sin `USING (true)`.
3. Añade índices para FKs lógicas y filtros frecuentes.
4. Registra la migración:

```bash
npm run db:migrations -- --update
npm run db:rls
```

5. Añade tests de integración con PostgreSQL efímero (happy path + aislamiento
   por empresa/negocio + error 400/401/403/404/409).

## 4. Runtime: salud, métricas y arranque

En `http/mod.rs`:

```rust
Router::new()
    .route("/health", get(health))   // liveness: no toca BD
    .route("/ready", get(ready))     // readiness: SELECT 1 + ping broker
    .route("/metrics", get(metrics)) // Prometheus
```

En `main.rs`:

- `Config::from_env()` y `validar_arranque()`;
- `TcpListener::bind(("0.0.0.0", puerto))`;
- `axum::serve(...).with_graceful_shutdown(...)`;
- cierre ordenado del pool, outbox y consumidor.

En `metrics.rs`, registra al menos:

- `core_engine_http_requests_total`
- `core_engine_http_request_duration_seconds`
- errores 5xx y 4xx relevantes
- métricas de negocio propias (outbox pendiente, etc.)

En `logs.rs`, JSON con `servicio`, `nivel`, `request_id` y sin PII.

## 5. Eventos (si publica o consume)

### Publica

- Añade `outbox.rs` con la tabla `<schema>.outbox` y el relay.
- Inserta el evento en la **misma transacción** del cambio de negocio.
- Declara `events.publishes` en `service.json`.
- Añade métricas de outbox pendiente/fallos.

### Consume

- Añade `rabbit.rs` + consumidor idempotente por `event_id`.
- Declara la cola y las routing keys.
- Enruta a DLQ tras `REINTENTOS_MAXIMOS`.
- Declara `events.consumes` en `service.json`.

### Verifica

```bash
npm run architecture:report   # matriz de eventos actualizada
npm run rules:check -- --only services-contract
```

## 6. Docker y compose

1. Copia `packages/Dockerfile.template` a
   `packages/example-service/Dockerfile` y sustituye `<servicio>` y `<puerto>`.
2. Añade el servicio a los composes de sus entornos:
   - `docker-compose.dev.yml`
   - `docker-compose.stage.yml`
   - `docker-compose.yml` (sólo si va a producción)
3. Declara `PORT` y `OUTBOX_TABLA` exactamente como en el manifiesto.
4. No publiques el puerto al host; sólo el gateway lo hace.

## 7. Gateway e identidad

### Gateway

- Añade el destino y la política por ruta/rol en `packages/api-gateway`.
- Añade el servicio a `readiness.rs` si debe participar en `/ready`.
- Añade la URL interna a la configuración del gateway y al compose.
- Prueba: ruta pública enruta; ruta interna no se expone.

### Identidad

- Si el servicio está gateado por capacidad, siembra la capacidad en
  `identity` (vertical/pack/empresa).
- Añade el gating en el servicio (`CapabilityGuard`/headers).
- Prueba 403 `CAPACIDAD_NO_ACTIVA` y happy path.

## 8. Contrato y documentación

- Implementa el envelope `{data,meta}` y los errores normalizados.
- Añade las rutas al OpenAPI del servicio.
- Genera y versiona:

```bash
npm run swagger:export
git diff --exit-code docs/openapi docs/api
```

- Escribe `packages/example-service/README.md` con:
  - bounded context y capacidades;
  - tablas/esquema que posee;
  - endpoints públicos/internos;
  - eventos que publica/consume;
  - variables de entorno;
  - comandos `cargo` y tests.
- Regenera el catálogo:

```bash
npm run architecture:report
```

## 9. Tests y calidad

| Capa | Mínimo |
|---|---|
| Unit | Casos de uso, validaciones, errores, dinero/fechas |
| Integración | API + PostgreSQL real: happy path, aislamiento, 400/401/403/404/409 |
| Contrato | Envelope, errores, OpenAPI, eventos |
| E2E | Flujo crítico en `npm run demo` si toca checkout |
| Cobertura | Umbral del manifiesto, sin bajar |

Comandos:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo llvm-cov --locked --fail-under-lines <N> --fail-under-functions <M>
```

## 10. Verificación final

```bash
npm run rules:check
npm run architecture:check
npm run baseline:guard
npm run test:engine
```

Y en CI:

- `matrix` deriva la matriz del nuevo `service.json`;
- `rust` compila, testea y mide cobertura;
- `docker-build` construye la imagen;
- `build` verifica OpenAPI y arquitectura.

Si algo falla, el mensaje indica servicio, archivo y remediación. No se mergea
en rojo.

## 11. Checklist de PR

- [ ] ADR + índice actualizado.
- [ ] `service.json` + catálogo generado.
- [ ] Crate con perfil/módulos.
- [ ] Migración + RLS + manifiesto de hashes.
- [ ] `/health`, `/ready`, `/metrics`, logs JSON, `x-request-id`.
- [ ] OpenAPI generado.
- [ ] Eventos declarados y probados.
- [ ] Capacidad sembrada y gating probado.
- [ ] Ruta/política del gateway y readiness.
- [ ] Compose dev/stage/prod.
- [ ] README del paquete.
- [ ] Tests unit/integración/contrato.
- [ ] SLO y alertas.
- [ ] `architecture:check` y `baseline:guard` verdes.
- [ ] Deuda nueva: cero.

## 12. Qué no hacer

- No copies un servicio entero “y luego lo adaptas”: arrastra deuda y dominio
  ajeno. Usa el esqueleto y el manifiesto.
- No reutilices el esquema de otro servicio “para no migrar”.
- No añadas una tabla al esquema de otro servicio.
- No crees un paquete compartido de runtime sin ADR.
- No añadas el servicio a `SERVICES` ni a la matriz de CI a mano: ya no existe
  esa lista; el manifiesto es la fuente.
- No edites a mano `catalogo-servicios.md`, `matriz-eventos.md` ni
  `deuda-arquitectura.md`: se regeneran.
