# Revisión integral — Core Engine (2026-11-08 → cierre 2026-11-10)

> Auditoría independiente sobre `feat/pack-builder` (`bb602b5`), ejecutada en un
> worktree limpio. Cubre lógica de negocio, arquitectura, tests, scripts y
> documentación. Los hallazgos se corrigieron y se validaron con la suite
> completa. La fotografía de cumplimiento vive en
> [`docs/rules/_meta/AUDIT.md`](../rules/_meta/AUDIT.md) (15ª auditoría).

## 1. Veredicto

Core Engine es una plataforma de comercio **multi-vertical** con gateway + 11
microservicios NestJS, PostgreSQL/Supabase, RabbitMQ y MinIO. El núcleo de
tests aporta valor real y, tras el cierre de hallazgos, **todos los gates
automáticos de PR están en verde**: 1.391 tests en 13 workspaces, cobertura
13/13 sobre el baseline, OpenAPI sin cambios incompatibles y build/lint limpios.

Quedan deudas **por diseño**, no fallos: envelope `/api/v2`, integración de la
saga completa, E2E en cada PR, acoplamiento por esquema entre algunos servicios
y 12 moderates de tooling.

## 2. Qué es el proyecto

| Servicio | Puerto | Responsabilidad |
|---|---|---|
| `api-gateway` | 8080 | Único punto de entrada: JWT HS256, políticas por ruta/rol, proxy, CORS, rate limit, readiness. |
| `identity-service` | 3001 | Auth/sesiones, usuarios, empresas, invitaciones, configuración, capacidades, packs, extensiones y partes. |
| `catalog-service` | 3002 | Productos/variantes, servicios, inventario, combos, promociones; reserva atómica de stock. |
| `stores-service` | 3003 | Tiendas, storefront, ofertas y sincronización de stock visible. |
| `orders-service` | 3004 | CQRS/Event Sourcing, carrito, proyecciones y transiciones. |
| `logistics-service` | 3005 | Envíos, asignación y avance de estados. |
| `scheduling-service` | 3011 | Recursos, disponibilidad, bloqueos y citas. |
| `commissions-service` | 3006 | Comisión por línea, pagos y liquidaciones (stage). |
| `finance-service` | 3007 | Partida doble, facturación, tributación y KPIs. |
| `field-service` | 3008 | Operación de campo por tenant. |
| `market-intelligence-service` | 3009 | Analítica/PostGIS (dev). |
| `storage-service` | 3010 | Buckets por tenant, URLs prefirmadas y cuotas. |

Flujo principal: `order.created` (orders) → reserva de stock (catalog) →
`stock.reservado` → `pagada`/`payment.procesado` (orders) → envío (logistics) →
`order.completado` → comisión (commissions) → asiento (finance), con outbox
transaccional y DLQ.

## 3. ¿Los tests aportan?

**Sí, el núcleo aporta.** Matriz final: **1.391 tests / 139 archivos / 13
workspaces**. Buen valor en dinero/estados, matriz de roles del gateway, stock
atómico, auth/sesiones, storage, logística, contabilidad e integración API+BD.

Se corrigieron los huecos que impedían el gate:

- `OutboxService` y `PgService` ya tienen spec (baseline de specs vacío).
- `identity-service` recuperó 93,20/84,68/95,45 con specs de `capacidades`,
  `extensiones` y `partes`.
- Se añadieron regresiones para el event store (versión única/reintento), el
  runtime de secretos y las rutas de autenticación.
- La integración es hermética: `catalog-items` aplica `01c` antes de `01e`.

Siguen siendo tests de bajo valor (deuda de calidad, no gate): asserts de
constantes, el `Proxy` de `comercio.controller.spec.ts` y varios caminos
positivos con `resolves.toBeDefined()`. No se eliminaron para no romper el
ratchet sin una decisión de producto; están listados en `AUDIT.md`.

## 4. ¿Los scripts aportan?

El **motor de gobernanza** es la mejor pieza del repo: suites declarativas,
manifiesto, descubrimiento aislado, centinela de falso verde y autotests. En el
cierre: `rules:check` 59/59, `test:engine` 24, `test:smoke` 62 y
`baseline:guard` sin retroceso.

El **QA harness** estaba roto y se corrigió: entrypoint `.js`, `bcryptjs`
declarado, sin falsos verdes, opciones CLI efectivas y lectura de profundidad
de colas. Sigue siendo manual (no está en CI) y su suite `15-static` duplica el
pipeline, lo cual está documentado.

## 5. Hallazgos y estado

### Corregidos (con regresión)

| Hallazgo | Corrección |
|---|---|
| `POST /auth/cambiar-contrasena` y `/auth/vincular-personal` devolvían 403 | Políticas de gateway con roles autenticados + tests |
| `/inteligencia/me/*` devolvía datos de todos los vendedores | Exige `x-user-personal`; agregación idempotente y transaccional |
| `POST /inteligencia/ventas/registrar` aceptaba `any` y roles amplios | DTO validado, vendedor forzado del token, roles acotados |
| `JWT_SECRET`/`ADMIN_PASSWORD` con default inseguro | Fallan en producción si faltan; defaults solo en dev |
| Outbox con doble publicación y ventana de 30 min | Reclamo atómico con `bloqueado_en` + lease + `SKIP LOCKED` |
| Event store sin versión única | Índice único + reintento con savepoint |
| Saga con efectos confirmados antes del outbox | Stock+outbox+idempotencia en una tx; pago en la tx de la transición; comisiones en una tx |
| RLS de `orders.proyeccion_ordenes` vs `orden_vista` | Políticas sobre `orden_vista`; contexto en consumidores |
| Comisión 12 % hardcodeada en tres sitios | `COMMISSION_RATE` + `comision_cents` en el evento (migración 46) |
| Cuota/borrado de storage con carreras | Lock de bucket en confirmación; borrado lógico antes de MinIO |
| Cita/bloqueo de scheduling con carrera | `FOR UPDATE` del recurso + traducción de `23505` |
| RabbitMQ: JSON fuera del try y ack antes de reinyectar | JSON inválido a DLQ; reinyección confirmada antes de ack |
| Finance omitía al empresario y estimaba la devolución | Cuenta `2.1.6` y reverso exacto del asiento previo |
| `contract:breaking` rojo por campos nuevos | Campos opcionales en OpenAPI; gate verde con 0 excepciones |
| Tests de identity bajo umbral | Specs de controllers y ramas de servicio |
| QA harness roto / falsos verdes | Entrypoint, dependencia, conteo por `pass`, CLI y colas |
| Integración dependiente del orden | `catalog-items` aplica `01c` antes de `01e` |

### No son fallos (decisiones/documentadas)

- **Acoplamiento entre esquemas**: orders lee `identity.empresa_capacidades`,
  scheduling/field leen `identity.partes` e identity escribe `stores.tiendas`.
  Es una integración por BD deliberada; cambiarla exige un rediseño de contratos.
- **`CapabilityGuard` parcial**: solo catalog/stores/scheduling/identity lo
  montan. Ampliarlo a más servicios es una decisión de producto.
- **Código/colas sin uso**: `reproyectar` es un no-op y algunas colas no tienen
  consumidor; no afectan los gates.
- **12 moderates de tooling**: no hay fix sin bajar ESLint 10→9 (major); el gate
  R-QA-6 exige `high` (0).
- **44 warnings `no-explicit-any`**: deuda de tipado, no error de lint.

## 6. Estado de gates (2026-11-10)

| Gate | Estado |
|---|---|
| G-1 unit | ✅ 1.391/1.391 |
| G-2 frontend | N/A |
| G-3 cobertura | ✅ 13/13 sobre threshold y baseline |
| G-4 integración | ✅/🟡 14 tests, faltan flujos de saga |
| G-5 contrato | ✅/🟡 `contract:breaking` verde; envelope `/api/v2` pendiente |
| G-6 E2E | ✅/🟡 release/manual |
| G-7 lint/build | ✅ 0 errores; build 0 |
| G-8 OpenAPI | ✅ 334 operaciones, sin diff |

## 7. Documentación alineada

- `README.md`: 11 microservicios, `scheduling:3011`, 13 workspaces,
  1.391 tests, 334 operaciones OpenAPI, readiness 9/10 y variables de entorno.
- `docs/rules/test/{06,07,README}` y `_meta/{AUDIT,AUDIT-HISTORY,CHECKLIST}`:
  conteos, gates y estado real.
- `.github/README.md`, `docs/runbook-entrega.md`, `docs/dockploy-setup.md`,
  `docs/decisiones.md`, `docs/rules/db/02-supabase.md`.
- `.env.stage.example` y `.env.production.example`: MinIO público 443/HTTPS.
- `qa-harness/README.md` y `qa-harness/package.json`.

## 8. Evidencia ejecutada

```text
npm ci                                      → 0
npm run lint                                → 0 (44 warnings)
npm run build                               → 0
npm test                                    → 0 (1.391 tests, 13 workspaces)
npm run coverage:compare                    → 0 (13/13 ≥ baseline)
npm run swagger:export                      → 0 (sin diff; 310 + 24 ops)
npm run contract:breaking -- --base-ref origin/main → 0 (0 excepciones)
npm run rules:check                         → 0 (59 tests, 11 suites)
npm run test:engine                         → 0 (24 tests)
npm run test:smoke                          → 0 (62 tests)
TEST_DATABASE_URL=... npm run test:integration → 0 (14/14, PostgreSQL 16)
npm run baseline:guard                      → 0
npm run db:migrations                       → 0 (49 migraciones)
npm run db:rls                              → 0 (61 tablas)
npm audit --audit-level=high                → 0
npm audit --audit-level=moderate            → 12 (tooling)
```

## 9. Recomendaciones siguientes (no bloqueantes)

1. Migrar el envelope de éxito a `/api/v2` para cerrar G-5.
2. Añadir una prueba de integración de la saga completa order→stock→pago→
   comisión→asiento.
3. Elevar el ratchet de `shared` y `market-intelligence` hacia 80/70.
4. Sustituir los tests de delegación por asserts de estado/montos.
5. Evaluar la bajada a ESLint 9 para eliminar los 12 moderates cuando haya
   ventana de mantenimiento.
