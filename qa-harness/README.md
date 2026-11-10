# Core Engine QA Harness

Herramienta **manual** de testing agresivo contra un stack Core Engine ya
levantado (API Gateway + microservicios + PostgreSQL + RabbitMQ + MinIO).
Genera reportes HTML, Markdown y JSON.

> **No es un gate de CI.** Se ejecuta a demanda contra `docker-compose.dev.yml`
> (o un entorno con credenciales equivalentes). El gate automático de cada PR es
> `npm test` de la raíz y los workflows de `.github/workflows/`.

## Suites (22)

| Suite | Descripción |
|-------|-------------|
| `00-preflight` | Health del gateway (el runner lo ejecuta como preflight) |
| `01-auth` | Autenticación y sesiones (RF-01, RF-02, RF-03) |
| `02-role-access` | Control de acceso y roles (ACL Tabla 21) |
| `03-catalog` | Catálogo unificado en items: producto, servicio y alquiler (RN-01, RN-02, RN-08) |
| `04-stores` | Tiendas y ofertas (RN-02, RN-03) |
| `05-orders` | Órdenes y saga (TC-03, TC-04, máquina de estados) |
| `06-cart` | Carrito y checkout (RN-05) |
| `07-logistics` | Logística y envíos |
| `08-commissions` | Comisiones y liquidaciones (RN-04, RN-06, RN-07) |
| `09-finanzas` | Finanzas, contabilidad, régimen fiscal NIC |
| `10-admin` | Reportes admin e inventario |
| `11-fuzzing` | Fuzzing y validación agresiva (SQLi, XSS, tipos) |
| `12-concurrency` | Concurrencia y race conditions (oversell, double submit) |
| `13-load` | Carga y rendimiento (p50/p90/p99, throughput) |
| `14-queue` | Outbox, DLQ, RabbitMQ, mensajería |
| `15-static` | Análisis estático (lint, build, tests, deuda) — lento y duplica CI |
| `16-owasp` | OWASP Top 10 |
| `17-schema` | Contratos de datos y esquemas de BD |
| `18-field` | Field service (backend móvil) |
| `19-identity` | Identity service (auth, usuarios, altas administrativas) |
| `20-multitenant` | Aislamiento por tenant |
| `21-market-intelligence` | Inteligencia de mercado, analítica y geoespacial |

## Bootstrap de datos (ADR-25/27)

El runner prepara el contexto multitenant antes de las suites:

1. Login del `super_admin` sembrado.
2. Registro/login de un **empresario** QA y alta (o recuperación) de la empresa
   `qa-harness` con el pack `pack_retail`.
3. Alta del **vendedor** dentro de esa empresa (participación 10 %) y creación de
   su tienda. Así el JWT lleva `tenant_id` y `participacion_porcentaje`.
4. Registro del comprador y login del usuario de logística sembrado por DB.

El registro público solo admite `empresario` y `comprador`; un `vendedor` por esa
vía responde `400`. La empresa y la tienda se reutilizan entre corridas.

## Instalación

```bash
cd qa-harness
npm install
```

Requiere Node >= 20 y el stack levantado:

```bash
docker compose -f docker-compose.dev.yml up -d --build
```

## Uso

```bash
# Todas las suites (reporte completo)
npm run test:full

# Suites concretas
npm start -- --suites 01-auth,02-role-access,03-catalog --format html

# Carga personalizada
npm start -- --load agresivo --concurrency 100 --fuzz 20

# Sin limpieza de datos
npm start -- --no-cleanup

# Ayuda
npm start -- --help
```

El entrypoint real es `src/index.js`. Los scripts npm ya apuntan a él.

## Opciones CLI

| Opción | Descripción | Default |
|--------|-------------|---------|
| `-u, --base-url` | URL base del API Gateway | `http://localhost:8080` |
| `-s, --suites` | Suites a ejecutar (coma-separadas) | Todas |
| `-f, --format` | Formato: `html`, `md`, `json`, `all` | `all` |
| `-l, --load` | Nivel carga: `agresivo`, `moderado`, `ligero` | `agresivo` |
| `-c, --concurrency` | Concurrencia para carga | `50` |
| `--fuzz` | Rondas de fuzzing | `10` |
| `--no-cleanup` | No limpiar datos de prueba | `false` |
| `-o, --output` | Directorio de reportes | `./reports` |

Las opciones se aplican a la configuración que consume el `Runner`; `--format`
decide qué archivos se escriben y `--output` dónde.

## Reportes

```
reports/
├── report.html    # Reporte interactivo con métricas, gráficos y evidencia
├── report.md      # Markdown para documentación
└── report.json    # Machine-readable para automatización
```

El código de salida es `1` si hay cualquier fallo de suite o de test. Una suite
que devuelve `pass:false` sin resultados (por ejemplo, por falta de token)
cuenta como fallo, no como verde.

## Variables de entorno

El harness lee `.env` del proyecto padre (`../../.env`). Variables relevantes:

```env
BASE_URL=http://localhost:8080
ADMIN_EMAIL=admin@core-engine.test
ADMIN_PASSWORD=AdminCore Engine2026!
DATABASE_URL=postgres://core_engine:core_engine_dev@postgres:5432/core_engine
INTERNAL_API_KEY=dev_internal_key
QA_TIMEOUT=30000
QA_CONCURRENCY=50
QA_FUZZ_ROUNDS=10
QA_LOAD_LEVEL=agresivo
QA_CLEANUP=true
QA_FORMAT=all
```

Las credenciales de desarrollo anteriores son las de `.env.example`; en un
entorno real se sobreescriben por variables de entorno.

## Arquitectura

```
qa-harness/
├── src/
│   ├── index.js              # Entry point CLI
│   ├── config.js             # Configuración desde ../../.env
│   ├── utils.js              # Clientes HTTP, RabbitMQ (Management API) y PostgreSQL
│   ├── runner/runner.js      # Orquestador de suites
│   ├── reporter/reportes.js  # Generador HTML/MD/JSON
│   └── suites/               # 22 suites (00-preflight … 21-market-intelligence)
└── package.json
```

## Limitaciones conocidas

- Requiere un stack real; no sustituye a `npm test`, `test:integration` ni al
  E2E de `e2e.yml`.
- `15-static` ejecuta `npm test`, `npm run lint` y `npm run build`: es lento y
  duplica el CI; úsalo solo a demanda.
- No está cubierto por ESLint (`eslint.config.mjs` solo lintea
  `packages/*/src/**/*.ts` y `scripts/**/*.mjs`).
- No se ejecuta en ningún workflow; es una herramienta de operación manual.

## Licencia

MIT — Core Engine Team
