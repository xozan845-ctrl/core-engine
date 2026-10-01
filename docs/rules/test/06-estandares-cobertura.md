# 06 — Estándares de Cobertura y Gates de CI

## Matriz de capas obligatorias por tipo de cambio

Todo PR debe incluir las capas marcadas con ✅ para el tipo de cambio tocado.

| Tipo de cambio | Unit | Integración (API+BD) | Contrato | E2E |
|---|---|---|---|---|
| Función util / VO / entidad | ✅ | — | — | — |
| Use-case / lógica de dominio | ✅ | ✅ (al menos 1 flujo que lo ejercite) | — | — |
| Endpoint nuevo o con filtros/paginación | ✅ (controller) | ✅ (filtros exactos, auth, errores) | ✅ | — |
| Endpoint con cambio de shape | ✅ | ✅ | ✅ (schema actualizado) | — |
| Flujo de usuario nuevo | ✅ (componente/servicio) | ✅ (si consume endpoint) | — | ✅ |
| **Bug en producción** | ✅ regression | ✅ si es de integración/filtros | si aplica | si es flujo visible |
| Cambio de zona horaria/fecha | ✅ boundary local vs UTC | ✅ día local completo | ✅ doc de semántica | ✅ R-E-4 |

> Las columnas Integración/Contrato/E2E son exigibles cuando la historia toque
> esas capas; hoy el repo aún no tiene suites de esas capas (gates G-4, G-5 y
> G-6 figuran como pendientes abajo). La fila "Flujo de usuario nuevo" aplica
> cuando exista UI en el repo (hoy no la hay).

## Umbrales de cobertura (workspaces con suite — Jest)

| Métrica | Mínimo global (objetivo) |
|---|---|
| Lines | ≥ 80% |
| Branches | ≥ 70% |
| Functions | ≥ 80% |

- Cobertura medida con `npm test -- --coverage` (lo que ejecuta el job `test` de CI); los números se registran en `AUDIT.md` en cada auditoría (R-COV-3).
- El gate mecánico actual son los `jest.coverageThreshold` de `packages/*/package.json` (`lines: 80`, `branches: 80`), presentes en los 6 workspaces con suite: `shared`, `api-gateway`, `orders-service`, `commissions-service`, `finance-service` y `market-intelligence-service`. Los otros workspaces no tienen suite (deuda, ver [`README.md`](./README.md)).
- **Anti-inflado**: subir cobertura no puede lograrse agregando asserts triviales; el gate real es la mutación (R-MT-2).
- **Ratchet (gate efectivo)**: la tabla es el objetivo final; el baseline se mide con el alcance de R-COV-4 redondeado a la baja (1 pt de margen de varianza) y sube **+5 puntos por release** hasta la tabla (tope: los mínimos de la tabla). Nunca baja (R-COV-1). Cada subida queda registrada en `AUDIT.md` (R-COV-3) y se vierte subiendo el `coverageThreshold` del workspace correspondiente.

## Umbrales de cobertura (frontend)

**N/A en este repo**: no existe `frontend/` ni suite con Karma, así que este
subapartado y `R-COV-5` quedan **reservados sin reutilizar el ID**. Si algún
día hay UI, su ratchet sigue el mismo método que el de Jest (baseline redondeado
a la baja, +5/release, tope en la tabla, nunca baja — R-COV-1).

## Gates de CI (bloquean el merge)

Estado verificado contra `.github/workflows/ci.yml` (único workflow existente):
✅ = job existe y bloquea · ⏸ = regla vigente, paso aún no configurado (deuda en
[`README.md`](./README.md)) · N/A = no aplica a este repo (ID reservado, nunca
se reutiliza).

| Gate | Condición | Estado |
|---|---|---|
| G-1 | Suite unit de todos los workspaces con tests en verde (100 %): job `test` (`npm test -- --coverage`), encadenado tras `security-gate`. | ✅ |
| G-2 | Suite unit frontend en verde. | N/A — no hay frontend. |
| G-3 | Cobertura ≥ `coverageThreshold` vigente en `packages/*/package.json#jest` (ratchet +5/release; alcance según R-COV-4): incumplirlo o bajar el umbral falla el job `test`. | ✅ |
| G-4 | Suite de integración API+BD en verde. | ⏸ sin suites de integración (exigible cuando existan, R-I-12). |
| G-5 | Test de contrato en verde. | ⏸ sin suites de contrato (R-C-7). |
| G-6 | E2E de flujos críticos en verde antes de release (puede ser etiqueta `release`, no cada PR). | ⏸ sin job E2E; lo más cercano hoy es el smoke local `docker compose up -d --build` + `npm run demo`. |
| G-7 | ESLint/TS sin errores: `npm run build` (job `build`) ✅; lint sin configurar (0 configs ESLint, ningún workspace expone script `lint`). | ⏸ parcial — build ✅, lint pendiente. |
| G-8 | Swagger spec sincronizado (R-C-7): `git diff --exit-code` tras el export. | ⏸ sin paso de export en CI. |

> `security-gate` (job `npm audit --audit-level=high` que encadena los demás) no
> es un `G-*`: es la regla R-QA-6.

## Anti-regresión de cobertura

| ID | Regla |
|---|---|
| R-COV-1 | Ningún PR puede **bajar** la cobertura global respecto a `main` (comparar reportes, no solo absoluto). |
| R-COV-2 | Archivos de lógica de negocio sin spec → bloquea merge (R-U-17). Se puede verificar con un script que liste los `.ts` ejecutables (alcance de R-COV-4) sin `*.spec.ts` par. |
| R-COV-3 | La auditoría `AUDIT.md` se actualiza en cada release con: cobertura actual, nº tests por capa, mutantes, deudas abiertas. Un gate solo se marca ✅ con evidencia de un **run verde de CI** (tener el paso configurado o pasar en local no basta). |
| R-COV-4 | **Alcance del gate**: solo se mide lógica ejecutable. Hoy ningún workspace declara `coveragePathIgnorePatterns`, así que el gate mide la suite tal cual está configurada. Si se acota el alcance, las exclusiones admitidas son: módulos Nest (`*.module.ts`, wiring DI), DTOs (`/dto/`, `/dtos/`), mocks/fixtures (`/mocks/`), scripts CLI y bootstrap (`main.ts`), y cada exclusión nueva se documenta en `AUDIT.md`. El baseline medido con ese alcance fija los thresholds del ratchet. |
| R-COV-5 | **N/A en este repo (sin frontend) — ID reservado, no reutilizar.** Regla heredada, exigible si algún día hay UI: el ratchet del frontend es norma, no una nota en el config — sigue el mismo método que el de Jest (baseline medido, redondeado a la baja con 1 pt de margen, +5 pts por release, tope en la tabla de umbrales y **nunca baja**, R-COV-1). Sin esta regla, G-3 protege solo los workspaces backend: la UI puede perder cobertura hasta el nivel del gate sin que nada lo note, porque un threshold obsoleto nunca falla. |
