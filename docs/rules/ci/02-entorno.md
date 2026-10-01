# 02 — Entorno y evidencia en CI

Aplica a: runner, dependencias, infraestructura de jobs y la evidencia que
deja cada ejecución.

| ID | Regla |
|---|---|
| R-EN-1 | **Entorno reproducible**: versión de Node fija en el workflow (hoy el literal `"20"` en el `setup-node` de cada job) y usada por todos los jobs, con cache de npm (`cache: npm`); instalación exclusivamente con `npm ci --prefer-offline` — el `package-lock.json` es la fuente de verdad y `npm install` libre está prohibido en CI. |
| R-EN-2 | **BD efímera por job (exigible cuando exista una suite que la necesite)**: los jobs actuales (`security-gate`, `lint`, `test`, `build`) no tocan ninguna BD. El día que un job ejecute suites de integración/E2E con BD (R-I-12), ese job levanta su propio servicio `postgres:16-alpine` con healthcheck, construye el esquema **solo desde el DDL versionado en `infra/db/init/*.sql`** (R-DB-6) y apunta `DATABASE_URL` únicamente a esa BD de job — CI jamás toca la BD de desarrollo (R-I-12). **Aplicar un esquema sin historial está prohibido en CI**: no deja rastro reproducible ni auditable, y acepta cambios de esquema que nadie ha revisado como SQL. |
| R-EN-3 | **Evidencia en todos los runs**: un job que genera reportes (hoy `test`, con el informe de cobertura) lo sube como artifact en **todos** los runs (`if: always()`), verde o rojo: es la evidencia de auditoría (R-COV-3). El estado del gate lo determina el resultado del job, nunca la presencia del artifact. |
| R-EN-4 | **Secretos**: viven únicamente en GitHub Secrets; prohibido imprimir valores sensibles en logs (`echo`, `set -x` sobre env de credenciales) y prohibido commitearlos — esta regla extiende R-GA-4 a CI. Ningún paso persiste credenciales entre jobs. |
