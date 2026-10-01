# 03 — Reglas de Tests E2E (stack completo: gateway → servicios → BD)

Aplica a: pruebas que recorren el stack completo con infraestructura real
(`docker compose`: gateway + servicios + Postgres + RabbitMQ). Hoy el repo **no
tiene frontend** (G-2 N/A): los flujos se ejercitan a nivel de API/CLI — el
precedente es el smoke `npm run demo` (`scripts/smoke.mjs`). Si algún día existe
UI, la capa pasa a navegador real (Playwright) conservando estos IDs.

## Flujos críticos (obligatorios antes de cada release)

| ID | Regla | Flujo |
|---|---|---|
| R-E-1 | Registro/login correcto: el token devuelto autentica `GET /auth/me` contra el perfil real de la BD. | auth |
| R-E-2 | Login con credenciales inválidas → `401` con shape de error, sin token. | auth |
| R-E-3 | Alta y publicación de producto + oferta: crear → visible en el catálogo → editar → despublicar. | catálogo |
| R-E-4 | **Filtro por fecha**: seleccionar un día con datos aserta que TODOS los registros listados pertenecen al día local seleccionado (y que un registro de día vecino NO aparece). | fechas |
| R-E-5 | Paginación conserva filtros y búsqueda. | listas |
| R-E-6 | Guard de rol: usuario sin rol (o sin token) en un endpoint restringido → `403` (`401` sin token). | authz |
| R-E-7 | Logout/refresh: el token viejo deja de servir (reuso → `401`). | auth |

## Calidad de ejecución

| ID | Regla |
|---|---|
| R-E-8 | **Cero errores en los logs de los servicios** (5xx, excepciones no capturadas) durante el flujo. Con UI: además cero errores de consola. Un error = test fallido. |
| R-E-9 | Cada flujo verifica el resultado por **datos** (cuerpo de la respuesta, conteos, estados en BD, URL), no solo por screenshots. Screenshots son evidencia complementaria. |
| R-E-10 | Con UI (cuando exista): selectores estables (`data-testid` preferido; CSS/roles como alternativa), prohibido depender de clases generadas por el build. Sin UI: verificación por datos de respuesta (R-E-9). |
| R-E-11 | **Suite hermética**: todo dato que un test necesita (usuarios, productos, registros boundary) viene del seed versionado en el repo (`infra/db/init/*.sql`, idempotente; fechas fijas con offset local `-06:00` explícito) — o lo crea la propia prueba vía API. Prohibido depender del estado manual de la BD local. Verificable: la suite completa pasa contra una BD recreada desde cero (init SQL), que es lo que ejecutará G-6 en CI cuando exista (hoy deuda). |
| R-E-12 | Ejecución headless en CI con retry único para red; timeout explícito por paso, no global largo. Los jobs de CI que arrancan la app fijan `TZ: America/Managua` (paridad con local, R-I-3). |
| R-E-13 | Los scripts E2E viven en el repo (no en `/tmp`), con `package.json` propio o script npm que instale sus dependencias. |

## Mínimo aceptable por release

- Suite E2E que ejecute R-E-1 … R-E-7 con pass rate 100%.
- Si un flujo crítico no puede automatizarse, queda registrado en `AUDIT.md` como deuda con responsable y fecha.
