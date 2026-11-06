# Core Engine QA Harness

Herramienta de testing agresivo y automatizado para el proyecto **Core Engine**.

## Características

- **Testing agresivo**: 21 suites cubriendo cada rincón de la aplicación
- **Reporte ultra detallado**: HTML interactivo, Markdown, JSON
- **Evaluación de control y calidad**: Scoring por dimensiones (seguridad, negocio, datos, rendimiento)
- **Pruebas de usuario**: Flujos completos comprador/vendedor/admin
- **Cobertura total**: Auth, ACL, CRUD, reglas de negocio, saga, concurrencia, fuzzing, estático

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
| `14-queue` | Outbox, DLQ, RabbitMQ, mensajería |
| `15-static` | Análisis estático (lint, build, tests, debt) |
| `16-owasp` | OWASP API Top 10 (JWT, BOLA, rate-limit, mass assignment) |
| `17-schema` | Contratos de esquema y validación de payloads |
| `18-field` | Flujos de field-service (asistencia, tracking, visitas) |
| `19-identity` | Identidad, sesiones y refresco de tokens |
| `20-multitenant` | Aislamiento entre tenants |
| `21-market-intelligence` | Reportes de market intelligence |

## Instalación

```bash
cd qa-harness
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

# Test de concurrencia
npm run test:concurrency

# Análisis estático
npm run test:static

# Outbox/DLQ/RabbitMQ
npm run test:queue

# Suites específicas
node src/index.js --suites 01-auth,02-role-access,03-catalog --format html

# Fuzzing personalizado
node src/index.js --suites 11-fuzzing --fuzz 20

# Sin limpieza de datos
node src/index.js --no-cleanup
```

## Opciones CLI

| Opción | Descripción | Default |
|--------|-------------|---------|
| `-u, --base-url` | URL base del API Gateway | `http://localhost:8080` |
| `-s, --suites` | Suites a ejecutar (coma-separadas) | Todas |
| `-f, --format` | Formato: `html`, `md`, `json`, `all` | `all` |
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

El harness lee `.env` del proyecto padre y da prioridad a las variables de
proceso. **No hay credenciales por defecto**: si falta una, el harness falla con
un mensaje explícito.

```env
BASE_URL=http://localhost:8080
ADMIN_EMAIL=<correo-del-admin>
ADMIN_PASSWORD=<clave-del-admin>
JWT_SECRET=<secreto-de-al-menos-32-chars>
DATABASE_URL=postgres://<usuario>:<clave>@localhost:5432/core_engine
INTERNAL_API_KEY=<clave-interna>
QA_TIMEOUT=30000
QA_FUZZ_ROUNDS=10
QA_CLEANUP=true
QA_FORMAT=all
```

Notas:

- `DATABASE_URL` del `.env` apunta a `postgres:5432` (red de Docker). Si ejecutas
  el harness **desde el host**, sobreescríbela con `localhost:5432`.
- `DATABASE_URL` del repo usa el rol de aplicación (`core_engine_app`). Las
  suites que escriben directo en la BD necesitan permisos y pueden quedar
  sujetas a RLS; el harness **no** valida RLS por sí mismo.
- `ADMIN_EMAIL`/`ADMIN_PASSWORD` son obligatorias para las suites de negocio.

## Requisitos

- Node.js >= 20 (usa v24 nativo: fetch, crypto, etc.)
- Stack Core Engine corriendo (`docker compose -f docker-compose.dev.yml up -d --build`)
- PostgreSQL y RabbitMQ accesibles

## Arquitectura

```
qa-harness/
├── src/
│   ├── index.js              # Entry point CLI
│   ├── config.js             # Configuración (.env + entorno)
│   ├── suites/               # 21 suites de testing
│   ├── reporter/
│   │   └── reportes.js       # Generador HTML/MD/JSON
│   ├── runner/
│   │   └── runner.js         # Orquestador de suites
│   └── utils.js              # Clientes HTTP, RabbitMQ, Postgres y JWT
└── package.json
```

## Filosofía "Roost-style"

Inspirado en **Roost.ai** (testing agent automatizado):
- **Descubrimiento automático**: Lee .env, healthchecks y OpenAPI
- **Generación de casos**: Suites predefinidas cubriendo specs (Tabla 21, RN-01..08, TC-01..08)
- **Ejecución agresiva**: Fuzzing, concurrencia, negativos
- **Evidencia completa**: Request/response/timing/traceId por test
- **Scoring de calidad**: Control + calidad = grade actionable

## Integración CI/CD

El harness **no** corre como gate de deploy: crea usuarios, órdenes y datos de
prueba, y algunas suites son agresivas. Su ejecución completa es manual contra un
entorno de pruebas o staging autorizado.

En CI solo se valida que el harness siga instalable y que su CLI arranque:

```yaml
# .github/workflows/ci.yml (job qa-harness)
- uses: actions/setup-node@v7
  with:
    node-version: "20"
    cache: "npm"
    cache-dependency-path: qa-harness/package-lock.json
- run: npm ci --prefer-offline
  working-directory: qa-harness
- run: npm run check
  working-directory: qa-harness
```

Para la ejecución real, usa un runner con acceso de red al entorno y define
`ADMIN_EMAIL`, `ADMIN_PASSWORD` y `DATABASE_URL` como secretos.

## Alcance y limitaciones

- Es QA **funcional/agresivo** (auth, ACL, negocio, fuzzing, concurrencia,
  colas, multitenant). La capacidad y la no-regresión de rendimiento se miden
  con k6, fuera de este harness; no dupliques carga aquí.
- Trabaja contra un stack real: no es una prueba unitaria ni un análisis
  estático. `15-static` sí ejecuta lint/build/tests del repo.
- El acceso directo a PostgreSQL es para *setup/teardown*, no para validar RLS.
  Usa el rol de servicio si necesitas saltar políticas y documenta el alcance.
- No lo ejecutes contra producción sin autorización escrita: crea datos y
  usuarios con contraseñas conocidas.

## Licencia

MIT - Core Engine Team