# 02 — Entorno y evidencia en CI

Aplica a: runner, dependencias, infraestructura de jobs y la evidencia que
deja cada ejecución.

| ID | Regla |
|---|---|
| R-EN-1 | **Entorno reproducible**: versión de Node fija en una variable de workflow (`NODE_VERSION`) usada por todos los jobs; pnpm con cache del store; instalación exclusivamente con `pnpm install --frozen-lockfile` — el lockfile es la fuente de verdad y `npm install` libre está prohibido en CI. |
| R-EN-2 | **BD efímera por job**: todo job que necesita BD levanta su propio servicio `postgres:16-alpine` con healthcheck, construye el esquema con `prisma migrate deploy` sobre las migraciones versionadas en `backend/prisma/migrations/` y apunta `DATABASE_URL` únicamente a esa BD de job — CI jamás toca la BD de desarrollo (R-I-12). **`db push` está prohibido en CI**: no deja historial, así que no es reproducible ni auditable, y acepta cambios de esquema que nadie ha revisado como SQL. Como `deploy` solo aplica lo que ya está versionado, el job `e2e` añade un `migrate diff --from-schema-datasource --to-schema-datamodel --exit-code` que detecta `schema.prisma` editado sin migración. |
| R-EN-3 | **Evidencia en fallo**: un job que genera reportes (Playwright, cobertura) los sube como artifact solo en `failure()` para diagnóstico; el estado del gate lo determina el resultado del job, nunca el artifact (R-COV-3). |
| R-EN-4 | **Secretos**: viven únicamente en GitHub Secrets; prohibido imprimir valores sensibles en logs (`echo`, `set -x` sobre env de credenciales) y prohibido commitearlos — esta regla extiende R-GA-4 a CI. Ningún paso persiste credenciales entre jobs. |
