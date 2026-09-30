# 06 — Estándares de Cobertura y Gates de CI

## Matriz de capas obligatorias por tipo de cambio

Todo PR debe incluir las capas marcadas con ✅ para el tipo de cambio tocado.

| Tipo de cambio | Unit | Integración (API+BD) | Contrato | E2E |
|---|---|---|---|---|
| Función util / VO / entidad | ✅ | — | — | — |
| Use-case / lógica de dominio | ✅ | ✅ (al menos 1 flujo que lo ejercite) | — | — |
| Endpoint nuevo o con filtros/paginación | ✅ (controller) | ✅ (filtros exactos, auth, errores) | ✅ | — |
| Endpoint con cambio de shape | ✅ | ✅ | ✅ (schema actualizado) | — |
| Pantalla/flujo de usuario nuevo | ✅ (componente/servicio) | ✅ (si consume endpoint) | — | ✅ |
| **Bug en producción** | ✅ regression | ✅ si es de integración/filtros | si aplica | si es flujo visible |
| Cambio de zona horaria/fecha | ✅ boundary local vs UTC | ✅ día local completo | ✅ doc de semántica | ✅ R-E-4 |

## Umbrales de cobertura (backend — Jest)

| Métrica | Mínimo global | Módulos críticos (`auth`, `attendance`, `devices`, `core/shared`) |
|---|---|---|
| Lines | ≥ 80% | ≥ 90% |
| Branches | ≥ 70% | ≥ 85% |
| Functions | ≥ 80% | ≥ 90% |

- Cobertura medida con `npm run test:cov`; los números se registran en `AUDIT.md` en cada auditoría.
- **Anti-inflado**: subir cobertura no puede lograrse agregando asserts triviales; el gate real es la mutación (R-MT-2).
- **Ratchet (gate efectivo)**: la tabla es el objetivo final; `coverageThreshold` de `jest.config.js` exige el baseline medido con el alcance de R-COV-4 redondeado a la baja (1 pt de margen de varianza) y sube **+5 puntos por release** hasta la tabla (tope: los mínimos de la tabla). Nunca baja (R-COV-1). Cada subida queda registrada en `AUDIT.md` (R-COV-3).

## Umbrales de cobertura (frontend)

- Mismo esquema que backend para `shared/`, `core/` y `features/**/services`.
- Componentes meramente visuales: smoke (render + inputs/outputs), no exigen 90%.
- El gate del ratchet es `check.global` de `coverageReporter` en `frontend/karma.conf.js` y lo ejecuta el job de frontend de CI con `ng test --code-coverage` (G-3, R-COV-5).

## Gates de CI (bloquean el merge)

| Gate | Condición |
|---|---|
| G-1 | Suite unit backend en verde (100% de tests). |
| G-2 | Suite unit frontend en verde. |
| G-3 | Cobertura ≥ `coverageThreshold` vigente en `backend/jest.config.js` **y** `check.global` de `frontend/karma.conf.js` (ratchet +5/release hacia la tabla; alcance según R-COV-4, frontend según R-COV-5). |
| G-4 | Suite de integración API+BD en verde. |
| G-5 | Test de contrato en verde. |
| G-6 | E2E de flujos críticos en verde antes de release (puede ser etiqueta `release`, no cada PR). |
| G-7 | ESLint/TS sin errores (ya existe). |
| G-8 | Swagger spec sincronizado (R-C-7). |

## Anti-regresión de cobertura

| ID | Regla |
|---|---|
| R-COV-1 | Ningún PR puede **bajar** la cobertura global respecto a `main` (comparar reportes, no solo absoluto). |
| R-COV-2 | Archivos de dominio sin spec → bloquea merge (R-U-17). Se puede verificar con script que liste `domain/**/*.ts` sin `*.spec.ts` par. |
| R-COV-3 | La auditoría `AUDIT.md` se actualiza en cada release con: cobertura actual, nº tests por capa, mutantes, deudas abiertas. Un gate solo se marca ✅ con evidencia de un **run verde de CI** (tener el paso configurado o pasar en local no basta). |
| R-COV-4 | **Alcance del gate**: solo se mide lógica ejecutable. Excluidos con `coveragePathIgnorePatterns`: módulos Nest (`*.module.ts`, wiring DI), DTOs (`/dto/`, `/dtos/`), mocks/fixtures (`/mocks/`), scripts CLI (`test-device-connection.ts`), bootstrap (`main.ts`) y `/types/`. El baseline medido con ese alcance fija los thresholds del ratchet (ver arriba). |
| R-COV-5 | **El ratchet del frontend es norma, no una nota en el config**: `check.global` de `frontend/karma.conf.js` sigue el mismo método que el de `jest.config.js` — baseline medido, redondeado a la baja con 1 pt de margen, +5 pts por release, tope en la tabla de umbrales, y **nunca baja** (R-COV-1). Sin esta regla, G-3 protege solo el backend: el frontend puede perder cobertura hasta el nivel del gate sin que nada lo note, porque un threshold obsoleto nunca falla. |
