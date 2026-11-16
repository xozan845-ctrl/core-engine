# 06 — Estándares de Cobertura y Gates de CI

## Matriz de capas obligatorias por tipo de cambio

Todo PR debe incluir las capas marcadas con ✅ para el tipo de cambio tocado.

| Tipo de cambio | Unit | Integración (API+BD) | Contrato | E2E |
|---|---|---|---|---|
| Función util / VO / entidad | ✅ | — | — | — |
| Use-case / lógica de dominio | ✅ | ✅ (al menos 1 flujo que lo ejercite) | — | — |
| Endpoint nuevo o con filtros/paginación | ✅ (handler) | ✅ (filtros exactos, auth, errores) | ✅ | — |
| Endpoint con cambio de shape | ✅ | ✅ | ✅ (schema actualizado) | — |
| Flujo de usuario nuevo | ✅ | ✅ (si consume endpoint) | — | ✅ |
| **Bug en producción** | ✅ regression | ✅ si es de integración/filtros | si aplica | si es flujo visible |
| Cambio de zona horaria/fecha | ✅ boundary local vs UTC | ✅ día local completo | ✅ doc de semántica | ✅ R-E-4 |

> La columna E2E aplica cuando exista UI o un flujo de negocio completo sobre el
> stack; hoy el repo no tiene UI y el E2E se ejecuta como gate de release (G-6).

## Cobertura de los servicios Rust

Todos los servicios son crates Rust y se miden con `cargo-llvm-cov` en el job
`rust` de CI (matriz por servicio, con PostgreSQL/PostGIS efímero para las
suites de integración). El umbral vive en `.github/workflows/ci.yml` y es la
**fuente única** del gate:

```bash
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines <lines> --fail-under-functions <functions>
```

| Servicio | Líneas | Funciones |
|---|---:|---:|
| `identity-service` | 79 | 75 |
| `api-gateway` | 78 | 73 |
| `field-service` | 77 | 71 |
| `stores-service` | 76 | 76 |
| `notifications-service` | 74 | 74 |
| `commissions-service` | 72 | 68 |
| `scheduling-service` | 71 | 65 |
| `logistics-service` | 63 | 62 |
| `market-intelligence-service` | 63 | 60 |
| `orders-service` | 62 | 63 |
| `storage-service` | 54 | 52 |
| `finance-service` | 52 | 45 |
| `item-service` | 40 | 43 |
| `pricing-service` | 53 | 52 |
| `inventory-service` | 79 | 72 |

- **Alcance** (R-COV-4): todo `src/**`, excluyendo `main.rs` (bootstrap).
  `llvm-cov` no mide ramas: la métrica de branches queda **N/A** y se justifica
  aquí; no se inventa un número.
- **Ratchet**: ningún PR baja el umbral de su servicio. Mejorar la cobertura
  exige subir el valor en la matriz de CI en el mismo PR (R-COV-1). El objetivo
  es converger al 80/70 de R-QA-1; los servicios por debajo tienen suites de
  integración que ejercitan lo que la unitaria no alcanza.
- Los archivos con baja cobertura unitaria por depender de broker, base de
  datos o tiempo real (`rabbit.rs`, `outbox.rs`, workers y consumidores) se
  cubren con las suites PostgreSQL/HTTP del crate y con el smoke de stage
  (R-PR-8/R-CD-12). La división de módulos (R-MS-7) no cambia el alcance: el
  denominador sigue siendo `src/**`.

## Gates de CI (bloquean el merge)

Estado verificado contra `.github/workflows/` (`ci`, `deploy`, `e2e`, `promote`,
`release`):
✅ = job existe y bloquea · N/A = no aplica a este repo (ID reservado, nunca se
reutiliza).

| Gate | Condición | Estado |
|---|---|---|
| G-1 | Suites unitarias en verde: job `rust` (`cargo llvm-cov`) para los 13 servicios + `npm run test:engine` y `npm run test:smoke` para el motor de gobernanza. | ✅ |
| G-2 | Suite unit frontend en verde. | N/A — no hay frontend. |
| G-3 | Cobertura ≥ umbral de la matriz `rust` (ratchet por servicio; alcance según R-COV-4). Incumplir o bajar el umbral falla el job. | ✅ |
| G-4 | Suite de integración API+BD en verde. | ✅ PostgreSQL efímero cubre matriz multi-vertical, packs v1/v2, RLS, enforcement, pilotos y concurrencia de Outbox (`npm run test:integration`). |
| G-5 | Test de contrato en verde. | ✅ envelope `{data,meta}` en todo `/api/v2`; errores 400/401/403/404/409/500 verificados en runtime; R-C-1 comprobado sobre el OpenAPI público. |
| G-6 | E2E de flujos críticos en verde antes de release. | ✅ **gate de release**: `release.yml` llama a `e2e.yml` antes de `images`; levanta `docker-compose.dev.yml`, ejecuta `npm run demo` y verifica logs (R-E-1…R-E-8). Un fallo bloquea la publicación. |
| G-7 | Lint de scripts y build Rust sin errores: `npm run lint` (job `lint`) + `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` y compilación (jobs `rust`). | ✅ |
| G-8 | Swagger sincronizado (R-C-7): `git diff --exit-code` tras `npm run swagger:export`. | ✅ |

> `security-gate` (`security:secrets` + `npm audit --audit-level=high`) es la
> regla R-QA-6, no un `G-*`.

## Anti-regresión de cobertura

| ID | Regla |
|---|---|
| R-COV-1 | Ningún PR puede **bajar** la cobertura de su servicio respecto al umbral vigente en `.github/workflows/ci.yml`. El job `rust` falla si `cargo llvm-cov` queda por debajo. Mejorar la cobertura exige subir el umbral en el mismo PR; nunca se baja para hacer pasar un cambio. |
| R-COV-2 | Toda lógica de negocio nueva (función pública de dominio, handler con reglas, consumidor) llega con sus pruebas en el mismo PR (`mod tests` para unitarias, `tests/<dominio>_pg.rs`/`_http.rs` para integración). La suite `microservices-ci-contract` verifica que cada crate declare sus suites y su README (R-MS-8). |
| R-COV-3 | La auditoría `AUDIT.md` se actualiza en cada release con: cobertura actual por servicio, nº de tests por capa y estado de los gates. Un gate solo se marca ✅ con evidencia de un **run verde de CI**; tener el paso configurado o pasar en local no basta. |
| R-COV-4 | **Alcance del gate**: solo se mide lógica ejecutable. `cargo llvm-cov` mide `src/**` menos `main.rs` (bootstrap). Las pruebas en `tests/` no forman parte del denominador. Un módulo nuevo entra en el alcance automáticamente; no hay lista de inclusión que mantener. |
| R-COV-5 | **N/A en este repo (sin frontend) — ID reservado, no reutilizar.** Exigible si algún día hay UI: mismo método de ratchet que el backend (baseline medido, +5 por release, nunca baja). |
