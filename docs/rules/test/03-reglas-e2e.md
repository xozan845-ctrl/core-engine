# 03 — Reglas de Tests E2E (frontend → API → BD)

Aplica a: pruebas que usan un navegador real (Puppeteer/Playwright) contra el
stack completo (`ng serve` + NestJS + Postgres).

## Flujos críticos (obligatorios antes de cada release)

| ID | Regla | Flujo |
|---|---|---|
| R-E-1 | Login correcto redirige al dashboard y renderiza datos reales de la API. | auth |
| R-E-2 | Login con credenciales inválidas muestra error sin romper el formulario. | auth |
| R-E-3 | CRUD completo de empleado: crear → aparece en la lista → editar → perfil → eliminar. | employees |
| R-E-4 | **Filtro de asistencia por fecha**: seleccionar un día con datos aserta que TODOS los registros listados pertenecen al día local seleccionado (y que un registro de día vecino NO aparece). | attendance |
| R-E-5 | Paginación conserva filtros y búsqueda. | listas |
| R-E-6 | Guard de rol: usuario sin rol intenta acceder a ruta restringida → redirigido. | authz |
| R-E-7 | Logout limpia sesión (recargar no restaura acceso). | auth |

## Calidad de ejecución

| ID | Regla |
|---|---|
| R-E-8 | **Cero errores de consola** (`console.error`, excepciones no capturadas, 404/500 de red) durante el flujo. Un error de consola = test fallido. |
| R-E-9 | Cada flujo verifica el resultado por **datos** (texto de celdas, conteo de filas, URL), no solo por screenshots. Screenshots son evidencia complementaria. |
| R-E-10 | Los selectores deben ser estables (`data-testid` preferido; Selectors CSS/roles como alternativa). Prohibido depender de clases CSS generadas por el build. |
| R-E-11 | **Suite hermética**: todo dato que un test necesita (usuarios, empleados, registros boundary) viene de `prisma/seed.ts` — idempotente (upserts; fechas fijas con offset local `-06:00` explícito) — o lo crea la propia prueba vía API. Prohibido depender del estado manual de la BD local. Verificable: la suite completa pasa contra una BD recreada desde cero (`prisma db push` + seed), que es lo que ejecuta G-6 en CI. |
| R-E-12 | Ejecución headless en CI con retry único para red; timeout explícito por paso, no global largo. Los jobs de CI que arrancan la app fijan `TZ: America/Managua` (paridad con local, R-I-3). |
| R-E-13 | Los scripts E2E viven en el repo (no en `/tmp`), con `package.json` propio o script npm que instale sus dependencias. |

## Mínimo aceptable por release

- Suite E2E que ejecute R-E-1 … R-E-7 con pass rate 100%.
- Si un flujo crítico no puede automatizarse, queda registrado en `AUDIT.md` como deuda con responsable y fecha.
