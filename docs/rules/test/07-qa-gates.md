# 07 — Reglas Doradas de Pruebas y QA

Aplica a: cobertura como gate de CI, independencia de las suites, edge cases,
aislamiento y la puerta de seguridad del pipeline. Complementa
[`01-reglas-unit.md`](./01-reglas-unit.md)…[`06-estandares-cobertura.md`](./06-estandares-cobertura.md)
sin repetirlas: aquí vive lo que el pipeline exige **como política global**.

## Reglas

| ID | Regla |
|---|---|
| R-QA-1 | **Cobertura mínima aplicable por CI**: la cobertura (code coverage) debe estar configurada con `coverageThreshold` aplicado automáticamente. El umbral mínimo obligatorio para capas críticas (servicios de dominio, handlers de CQRS) es del **80%** en líneas y ramas. No se aceptan PRs que reduzcan la cobertura por debajo del umbral en áreas núcleo (pagos, inventario, estados de órdenes). El pipeline de CI debe fallar automáticamente si se viola. **Excepción documentada:** `packages/field-service` no tiene suite de pruebas (no entra en `npm test`); debe excluirse explícitamente del análisis de cobertura en la configuración del workspace hasta que se implemente su suite. |
| R-QA-2 | **Pruebas unitarias independientes**: las pruebas unitarias (`*.spec.ts`) jamás deben conectarse a una base de datos real ni a un broker de RabbitMQ físico. Deben usar mocks, stubs (ej. `jest.mock`) o repositorios en memoria, asegurando que corren en milisegundos. |
| R-QA-3 | **Pruebas de integración y end-to-end (E2E)**: son las únicas permitidas para probar la conectividad real con Postgres y RabbitMQ (usualmente orquestadas vía `docker-compose`). Deben ejecutarse en un entorno limpio y purgar sus datos tras finalizar para no dejar estado residual (flaky tests). |
| R-QA-4 | **Validación de casos de borde (edge cases)**: los tests no solo deben probar el "camino feliz" (happy path). Es obligatorio crear pruebas que fuercen errores como: saldo insuficiente, errores 409 (conflict), tokens expirados y mensajes no formateados, validando que el sistema responde de manera graceful. |
| R-QA-5 | **Aislamiento de tests**: cada caso de prueba `it(...)` debe ser completamente independiente del anterior. No se debe arrastrar estado global ni variables estáticas entre una prueba y otra. |
| R-QA-6 | **Puerta de seguridad en CI (security gate)**: el pipeline de CI/CD DEBE incluir `npm audit --audit-level=high` como paso obligatorio previo al despliegue. Si existen vulnerabilidades de nivel alto o crítico no resueltas, el pipeline debe fallar. Aplica `R-DS-7` (`../data/02-seguridad.md`) de forma automática y no dependiente de acción manual. |
