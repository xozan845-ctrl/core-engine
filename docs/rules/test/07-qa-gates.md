# 07 — Reglas Doradas de Pruebas y QA

Aplica a: cobertura como gate de CI, independencia de las suites, edge cases,
aislamiento y la puerta de seguridad del pipeline. Complementa
[`01-reglas-unit.md`](./01-reglas-unit.md)…[`06-estandares-cobertura.md`](./06-estandares-cobertura.md)
sin repetirlas: aquí vive lo que el pipeline exige **como política global**.

## Reglas

| ID | Regla |
|---|---|
| R-QA-1 | **Cobertura mínima aplicable por CI**: cada crate Rust tiene su umbral de `cargo llvm-cov` en la matriz `rust` de `.github/workflows/ci.yml` (R-COV-1). El objetivo para capas críticas (dominio, handlers, pagos, inventario, estados) es **80% de líneas** y **70% de funciones**; los servicios por debajo convergen con el ratchet y no pueden bajar. El pipeline falla automáticamente si un crate queda por debajo de su umbral. `microservices-ci-contract` verifica que los 13 servicios declaren crate, suites y README, y bloquea omisiones no declaradas en `SERVICE_TEST_EXCEPTIONS` (`scripts/config/project.mjs`). |
| R-QA-2 | **Pruebas unitarias independientes**: las pruebas de `mod tests` jamás se conectan a una base de datos real ni a un broker de RabbitMQ físico. Usan funciones puras, dobles de los traits de puerto o `serde_json` en memoria, y corren en milisegundos sin `DATABASE_URL`. |
| R-QA-3 | **Pruebas de integración y end-to-end (E2E)**: son las únicas permitidas para probar la conectividad real con Postgres y RabbitMQ (usualmente orquestadas vía `docker-compose`). Deben ejecutarse en un entorno limpio y purgar sus datos tras finalizar para no dejar estado residual (flaky tests). |
| R-QA-4 | **Validación de casos de borde (edge cases)**: los tests no solo deben probar el "camino feliz" (happy path). Es obligatorio crear pruebas que fuercen errores como: saldo insuficiente, errores 409 (conflict), tokens expirados y mensajes no formateados, validando que el sistema responde de manera graceful. |
| R-QA-5 | **Aislamiento de tests**: cada caso `#[test]`/`#[tokio::test]` debe ser independiente. No se arrastra estado global ni variables estáticas entre pruebas; las suites PostgreSQL crean y limpian sus datos (R-I-9). |
| R-QA-6 | **Puerta de seguridad en CI (security gate)**: el pipeline de CI/CD DEBE incluir `npm audit --audit-level=high` como paso obligatorio previo al despliegue. Si existen vulnerabilidades de nivel alto o crítico no resueltas, el pipeline debe fallar. Aplica `R-DS-7` (`../data/02-seguridad.md`) de forma automática y no dependiente de acción manual. |
