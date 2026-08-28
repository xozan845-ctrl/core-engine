# BodegaHub QA Harness

Herramienta de testing agresivo y automatizado para el proyecto **BodegaHub Core Engine**.

## Características

- **Testing agresivo**: 15 suites cubriendo cada rincón de la aplicación
- **Reporte ultra detallado**: HTML interactivo, Markdown, JSON
- **Evaluación de control y calidad**: Scoring por dimensiones (seguridad, negocio, datos, rendimiento)
- **Pruebas de usuario**: Flujos completos comprador/vendedor/admin
- **Cobertura total**: Auth, ACL, CRUD, reglas de negocio, saga, concurrencia, carga, fuzzing, estático

## Suites incluidas

| Suite | Descripción |
|-------|-------------|
| `00-preflight` | Verificación pre-ejecución (health, puertos, BD, RabbitMQ) |
| `01-auth` | Autenticación y sesiones (RF-01, RF-02, RF-03) |
| `02-role-access` | Control de acceso y roles (ACL Tabla 21) |
| `03-catalog` | Catálogo CRUD y reglas (RN-01, RN-02, RN-08) |
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
| `15-static` | Análisis estático (lint, build, tests, debt) |

## Instalación

```bash
cd core-engine/qa-harness
npm install
```

## Uso

```bash
# Ejecutar todas las suites (reporte completo)
npm run test:full

# Solo suites de seguridad
npm run test:security

# Solo reglas de negocio
npm run test:business

# Test de carga agresivo
npm run test:load

# Test de concurrencia
npm run test:concurrency

# Análisis estático
npm run test:static

# Outbox/DLQ/RabbitMQ
npm run test:queue

# Suites específicas
node src/index.ts --suites 01-auth,02-role-access,03-catalog --format html

# Carga personalizada
node src/index.ts --load agresivo --concurrency 100 --fuzz 20

# Sin limpieza de datos
node src/index.ts --no-cleanup
```

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

## Reportes generados

```
reports/
├── report.html    # Reporte interactivo con métricas, gráficos, evidencia
├── report.md      # Markdown para CI/CD y documentación
└── report.json    # Machine-readable para automatización
```

### HTML Report incluye:
- Scorecards por dimensión (Seguridad, Negocio, Datos, Rendimiento, Observabilidad)
- Tabla detallada por suite con ✅/❌
- Evidencia expandible: request/response, headers, timing, traceId
- Recomendaciones por control fallado
- Badge de calidad general (A+/A/B/C/D/F)

## Variables de entorno

El harness lee automáticamente `.env` del proyecto padre. Variables relevantes:

```env
BASE_URL=http://localhost:8080
ADMIN_EMAIL=admin@bodegahub.test
ADMIN_PASSWORD=AdminBodegaHub2026!
JWT_SECRET=dev_secret
DATABASE_URL=postgres://bodegahub:bodegahub_dev@postgres:5432/bodegahub
INTERNAL_API_KEY=dev_internal_key
QA_TIMEOUT=30000
QA_CONCURRENCY=50
QA_FUZZ_ROUNDS=10
QA_LOAD_LEVEL=agresivo
QA_CLEANUP=true
QA_FORMAT=all
```

## Requisitos

- Node.js >= 20 (usa v24 nativo: fetch, crypto, etc.)
- Stack Core Engine corriendo (`docker compose up -d --build`)
- PostgreSQL y RabbitMQ accesibles

## Arquitectura

```
qa-harness/
├── src/
│   ├── index.ts              # Entry point CLI
│   ├── config.ts             # Configuración desde .env
│   ├── client/
│   │   ├── http-client.ts    # HTTP client con fetch nativo
│   │   ├── rabbitmq.ts       # RabbitMQ Management API
│   │   └── postgres.ts       # PostgreSQL directo (pg)
│   ├── suites/               # 15 suites de testing
│   ├── reporter/
│   │   └── reportes.ts       # Generador HTML/MD/JSON
│   ├── runner/
│   │   └── runner.ts         # Orquestador de suites
│   └── utils.ts              # Utilidades compartidas
└── package.json
```

## Filosofía "Roost-style"

Inspirado en **Roost.ai** (testing agent automatizado):
- **Descubrimiento automático**: Lee .env, healthchecks, swagger
- **Generación de casos**: Suites predefinidas cubriendo specs (Tabla 21, RN-01..08, TC-01..08)
- **Ejecución agresiva**: Fuzzing, concurrencia, carga, negativos
- **Evidencia completa**: Request/response/timing/traceId por test
- **Scoring de calidad**: Control + calidad = grade actionable

## Integración CI/CD

```yaml
# .github/workflows/qa.yml
- name: QA Harness
  run: |
    cd core-engine/qa-harness
    npm install
    npm run test:full
    # report.html se puede publicar como artifact
```

## Licencia

MIT - BodegaHub Team