# rules/ — Reglas del proyecto (punto fijo)

Este directorio define las reglas **permanentes** del proyecto: el punto fijo al que
no se desvía ninguna decisión técnica. Las reglas no cambian por conveniencia de un
PR; cambian por decisión explícita y quedan registradas aquí.

## Estructura

```
rules/
├── README.md           ← este índice: áreas, estado, convenciones globales
│
├── architecture/       ✅ activa (sin auditar) — estilo arquitectónico y nomenclatura (R-AR/R-NC)
├── microservice/       ✅ activa (sin auditar) — estructura de servicios y excepción OLAP (R-MS/R-MI)
├── gateway/            ✅ activa (sin auditar) — perímetro, JWT y contrato externo (R-GW)
├── data/               ✅ activa (sin auditar) — integridad y seguridad de los datos (R-DI/R-DS)
├── db/                 ✅ activa (sin auditar) — Postgres y Supabase (R-DB/R-SB)
├── observability/      ✅ activa (sin auditar) — logs/trazas y métricas/alertas (R-OB/R-TM)
│
├── test/               ✅ activa — reglas de testing (R-U/R-I/R-E/R-C/…/R-QA)
├── git/                ✅ activa (sin auditar) — flujo git (R-GA/R-GC/R-GP/R-PR)
├── ci/                 ✅ activa (sin auditar) — CI y despliegue (R-CI/R-EN/R-CD)
│
└── _meta/              ⚠️ documentos DERIVADOS — nunca normativos
    ├── CHECKLIST.md        herramienta de revisión de PR
    ├── AUDIT.md            snapshot del estado (se reescribe por auditoría)
    └── AUDIT-HISTORY.md    log append-only de auditorías (procedencia: proyecto origen + Core Engine)
```

**Regla de estructura (la más importante de esta carpeta):**

> Un **área** (`architecture/`, `data/`, `db/`, `gateway/`, `microservice/`,
> `observability/`, `test/`, `git/`, `ci/`) solo contiene `README.md` + archivos de
> reglas `NN-tema.md`. Todo documento derivado (audit, checklist, log, estado) vive
> en `_meta/` y jamás dentro de un área.

## Áreas

| Área | Estado | Índice | Contenido |
|---|---|---|---|
| `architecture/` | ✅ **activa** (sin auditar) | [`architecture/README.md`](./architecture/README.md) | Arquitectura: CQRS/Event Sourcing por dominio, coreografía, outbox, DLQ, autonomía, caché TTL; naming de archivos/clases/tablas/commits (IDs `R-AR/R-NC`) |
| `microservice/` | ✅ **activa** (sin auditar) | [`microservice/README.md`](./microservice/README.md) | Estructura de servicios (clean architecture, DI, DTOs, fail-fast, shutdown) y excepción OLAP de `market-intelligence` (IDs `R-MS/R-MI`) |
| `gateway/` | ✅ **activa** (sin auditar) | [`gateway/README.md`](./gateway/README.md) | API Gateway: single entry point, edge offloading, JWT + scrub de headers, TTLs, RFC 7807, versionamiento (IDs `R-GW`) |
| `data/` | ✅ **activa** (sin auditar) | [`data/README.md`](./data/README.md) | Integridad (centavos, stock atómico, estados, idempotencia, partida doble) y seguridad (secretos, PII, DTOs, rate limiting, `npm audit`) (IDs `R-DI/R-DS`) |
| `db/` | ✅ **activa** (sin auditar) | [`db/README.md`](./db/README.md) | Postgres: schema-per-service, RLS, transacciones, índices, soft delete, migraciones; Supabase: paridad, Auth, secretos (IDs `R-DB/R-SB`) |
| `observability/` | ✅ **activa** (sin auditar) | [`observability/README.md`](./observability/README.md) | Logs JSON + `x-request-id`; métricas Prometheus, labels RED, dashboards as code, alertas (IDs `R-OB/R-TM`) |
| `test/` | ✅ **activa** ⚠️ snapshot heredado | [`test/README.md`](./test/README.md) | Reglas de testing: unit, integración, E2E, contrato, robustez, cobertura, QA gates (IDs `R-U/R-I/R-E/R-C/…/R-QA`) |
| `git/` | ✅ **activa** (sin auditar) | [`git/README.md`](./git/README.md) | Flujo git profesional: `git add`, `git commit -m`, `git push`, `gh pr` (IDs `R-GA/R-GC/R-GP/R-PR`) |
| `ci/` | ✅ **activa** (sin auditar) | [`ci/README.md`](./ci/README.md) | CI: gatillos push/PR, entorno, evidencia y despliegue (IDs `R-CI/R-EN/R-CD`) |
| `_meta/` | ⚠️ **no normativo** | [`_meta/README.md`](./_meta/README.md) | Derivados: checklist, snapshots de auditoría, histórico |

## Convenciones globales (aplican a todas las áreas)

1. **Archivos:** `README.md` (índice del área + mapa de IDs + pendientes) y reglas en
   archivos `NN-tema.md` con título `# NN — Tema`. Nunca más de un tema por archivo.
2. **IDs de regla:** `R-<SUFIXO>-<n>` secuencial e incremental, un sufijo por archivo,
   un archivo por área temática. Gates de CI: `G-<n>`. Un ID = una regla, para siempre
   (nunca renumerar ni reutilizar).
3. **Formato de regla:** tabla `| ID | Regla |`, redacción en imperativo y verificable
   en un PR. Si no se puede comprobar, no es una regla.
4. **Referencias:** siempre por ID completo (`R-U-17`), nunca por número suelto. Las
   referencias entre áreas incluyen la ruta (`R-AR-2` en `../architecture/01-arquitectura.md`).
5. **Regla ≠ estado:** la regla dice *qué*; `_meta/AUDIT.md` dice *qué tan cumplida está*.
   En cada auditoría se reescribe el snapshot y se agrega una entrada al histórico.
6. **Referencias cruzadas:** `grep -rn "R-XX-n" docs/rules/` antes de renombrar/eliminar.

El área `test/` es el ejemplo canónico al que apuntan las demás.

## Origen y deudas de esta reestructuración

- Las áreas `architecture/ data/ db/ gateway/ microservice/ observability/` nacieron
  de las "Reglas Doradas" que vivían sueltas en la raíz (heredadas de otro proyecto)
  y se convirtieron al formato `| ID | Regla |` con IDs propios al reorganizarse.
- **Pendiente:** `_meta/AUDIT.md`, `_meta/CHECKLIST.md` y `_meta/AUDIT-HISTORY.md`
  todavía son snapshots del proyecto origen (comandos `pnpm`, `backend/`, `prisma/`);
  re-auditar este repo y reescribirlos (R-COV-3).
