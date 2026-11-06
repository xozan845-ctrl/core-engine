# security — checks ejecutables de seguridad

Esta área es el **índice normativo** de las comprobaciones de seguridad que se
ejecutan como código. No duplica las reglas de producto de `data/`, `gateway/` o
`db/`: las suites viven en `scripts/rules/security/` y citan los IDs existentes.

Los artefactos operativos están separados:

- hardening de host: `infra/security/hardening/`
- informes de auditoría: `docs/security/reports/`
- pruebas de capacidad/no-regresión: `tests/k6/`

## Suites

Una suite por clase de vulnerabilidad. `npm run rules:check` (y el job
`rules-check` de CI) las ejecuta; el manifiesto en `scripts/config/manifest.mjs`
impide borrarlas en silencio.

| Suite | Qué verifica | Reglas |
| --- | --- | --- |
| `excessive-data-exposure` | campos internos en respuestas públicas (API3:2023) | `R-DS-3`, `R-C-3` |
| `secrets-hygiene` | plantillas sin secretos, sin fallback `dev_secret`, admin sin clave por defecto | `R-DS-1`, `R-CD-7` |
| `authz-matrix` | rutas públicas/protegidas, scrub de identidad y `/internal` no ruteado | `R-GW-1`, `R-GW-3`, `R-DS-5` |
| `headers-cors` | cabeceras de seguridad y CORS por whitelist | `R-GW-2` |
| `rate-limit` | cubo estricto de login, cubo global y `/ready` limitado | `R-DS-6` |
| `sql-injection` | interpolación SQL no paramétrica | `R-DS-4` |
| `ssrf` | `fetch` con URLs tomadas de la petición | `R-DS-5` |
| `uploads` | whitelist de tipos, SVG bloqueado, límites y saneado | `R-DS-4` |
| `mass-assignment` | `whitelist` + `forbidNonWhitelisted` en los DTOs | `R-DS-4` |
| `sensitive-logs` | contraseñas/tokens en logs | `R-OB-1`, `R-DS-2` |

## Cómo añadir una suite

1. Un archivo `.test.mjs` nuevo en `scripts/rules/security/`, con
   `defineSuite({ id: 'security-<tema>', area: 'security', rules: [...] })`.
2. Registrarlo en `scripts/config/manifest.mjs`.
3. Fallar con un mensaje que diga **qué** se filtró/incumplió y **dónde**.
4. Ejecutar `npm run rules:check` y actualizar la tabla de arriba.

## Alcance

Estos checks son estáticos/de repositorio: detectan regresiones en código,
contratos y configuración. No sustituyen a los tests de integración/E2E ni a
una revisión de seguridad puntual; para capacidad y no-regresión de rendimiento
está `tests/k6/` y para QA funcional/agresivo está `qa-harness/`.
