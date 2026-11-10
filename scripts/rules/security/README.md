# scripts/rules/security — verificaciones de seguridad

Un archivo `.test.mjs` por clase de vulnerabilidad. Cada suite usa
`defineSuite(...)`, se registra en `scripts/config/manifest.mjs` y cita las
reglas de `docs/rules/` que verifica. `npm run rules:check` (y el job
`rules-check` de CI) las ejecuta junto al resto del motor de reglas.

Estas suites son **checks estáticos/de repositorio**: detectan regresiones en
el código y en los contratos sin levantar el stack. No sustituyen a los tests
de integración/E2E ni a una revisión de seguridad puntual.

## Estado

| Archivo | Vulnerabilidad | Reglas | Estado |
| --- | --- | --- | --- |
| `excessive-data-exposure.test.mjs` | Campos internos en respuestas públicas (OWASP API3:2023) | `R-DS-3`, `R-C-3` | ✅ |
| `secrets-hygiene.test.mjs` | Secretos por defecto, placeholders y fallbacks | `R-DS-1`, `R-CD-7` | ⏳ |
| `authz-matrix.test.mjs` | Rutas públicas/protegidas y superficies internas | `R-GW-1`, `R-GW-3`, `R-DS-5` | ✅ |
| `idor-tenant.test.mjs` | Consultas sin filtro de tenant / IDOR | `R-DS-3`, `R-DB-2` | ⏳ |
| `rate-limit.test.mjs` | Cobertura de límites en login/registro/refresh | `R-DS-6` | ✅ |
| `headers-cors.test.mjs` | Cabeceras de seguridad y CORS | `R-GW-2` | ✅ |
| `sql-injection.test.mjs` | Interpolación de SQL no paramétrica | `R-DS-4` | ✅ |
| `ssrf.test.mjs` | `fetch` con URLs controladas por el usuario | `R-DS-5` | ✅ |
| `uploads.test.mjs` | Whitelist de tipos, SVG y tamaño | `R-DS-4` | ✅ |
| `mass-assignment.test.mjs` | DTOs estrictos (`whitelist`, `forbidNonWhitelisted`) | `R-DS-4` | ✅ |
| `sensitive-logs.test.mjs` | Tokens/contraseñas en logs | `R-OB-1` | ✅ |

## Convenciones

- Un archivo = una vulnerabilidad; no mezclar comprobaciones de temas distintos.
- Fallar con un mensaje que diga **qué campo/ruta** se filtró y **dónde**.
- Si una comprobación necesita una lista blanca, declararla en el propio
  archivo para que el revisor vea exactamente qué se permite.
- Las suites deben pasar en `develop`; si una dependencia aún no existe, se
  añade en el mismo PR que la introduce (no se dejan checks condicionales
  silenciosos).
