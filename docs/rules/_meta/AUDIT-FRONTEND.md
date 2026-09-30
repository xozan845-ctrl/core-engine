# AUDIT-FRONTEND — Cumplimiento de las reglas de frontend

> ⚠️ **Heredado del proyecto origen — NO APLICA a Core Engine.** Este repo no
> tiene área `frontend/` ni familia `R-ES`; los enlaces a `../frontend/` de
> abajo están muertos. Snapshot conservado hasta la primera auditoría de
> Core Engine (R-COV-3), momento en que se decide borrarlo.

> **Snapshot** reescrito completo en cada auditoría de frontend (R-COV-3): este archivo
> siempre describe el estado **presente** del código contra [`../frontend/`](../frontend/README.md).
> Las reglas normativas viven en las áreas; aquí solo hay estado, evidencia y deudas.

- **Última auditoría:** 2026-09-27 (familia completa); **2026-09-28: re-auditoría
  puntual de R-ES-18 sobre los 14 stores** (registro en consola), con fix de D-FE-6.
- **Alcance:** familia `R-ES` de [`04-estado-rxjs.md`](../frontend/04-estado-rxjs.md)
  (18 reglas), con consulta de las reglas hermanas que la delimitan (`R-ES-4` ↔ `R-ES-17`,
  `R-CP-3`, `R-FD-6`/`R-ES-5`, `R-ES-15`/`R-ES-16`, `R-E-8`).
- **Estado global:** ✅ **CUMPLE** — **18 ✅ / 0 🟡 / 0 ❌** de 18;
  sin deudas abiertas en la familia `R-ES` y gates de CI en verde (G-1/G-2).

## Resumen por regla

| ID | Estado | Evidencia |
|---|---|---|
| R-ES-1 | ✅ | 14 `*.store.ts`: `auth`, `toast` y 11 de feature en `core/stores/` y `features/*/stores/` (13 `signalStore(`; `base.store.ts` aporta los helpers `CollectionState`). Sin estado de dominio en `Subject` de servicios: 0 `*.service.ts` salvo `core/api/api-client.ts` (sin estado de negocio). |
| R-ES-2 | ✅ | Los CRUD siguen `CollectionState`/`withCollectionMethods`; `audit.store.ts:37-38` mapea fechas a ISO a mano — mapeo propio previsto por la regla. |
| R-ES-3 | ✅ | grep de mutación directa (`.items.push`, `.splice`, `.total++`): 0 — todo vía `patchState` desde dentro del store. |
| R-ES-4 | ✅ | Páginas leen con `inject(XStore)` + `computed()`; sin copias de dominio (el estado de vista son los filtros del patrón `employees-list`). El dashboard hoy solo arma su view-model con valores de stores (R-ES-17 ✓). |
| R-ES-5 | ✅ | `features/*` solo importan `core/`/`shared/` (grep de imports cruzados: 0). Las páginas importan varios stores porque están en capa superior (R-FD-6 no lo prohíbe). ⚠️ Observación de redacción: "prohibido importar el store de otra feature" debe leerse como feature→feature; conviene aclararlo en la regla. |
| R-ES-6 | ✅ | 0 `.subscribe()` en `src/` fuera de specs. `search-field.component.ts` usa `toSignal` (`controlValue`, `debouncedSearch`) + `effect` para `text` y `search`; `layout.component.ts` usa `toSignal(navigationEnd)` + `effect` para título y menú móvil. El spec de `search-field` añade el flush de effects (`detectChanges`) y 2 tests de sincronización. |
| R-ES-7 | ✅ | Sin suscripciones manuales en componentes que gestionar (R-ES-6: `toSignal` se destruye con la vista); el polling se detiene en `attendance-realtime.page.ts:95` (`onDestroy` → `stopRealtimePolling` → comando `null` que cancela el intervalo). |
| R-ES-8 | ✅ | 0 `subscribe` anidados y 0 `subscribe()` directos en stores: `device.store` `refreshServerLogs` es `rxMethod` + `mergeMap` (paralelo como el `subscribe` original) y `attendance.store` el polling es `rxMethod` con comando `{intervalMs,getFilters} \| null` (`switchMap` cancela el intervalo anterior; `null` = stop). |
| R-ES-9 | ✅ | 20 `effect()` en páginas revisados: patch de formulario desde store, toast de éxito/error, descarga de blob, título de ruta — todos efectos colaterales reales, ninguno deriva valores (R-ES-9). |
| R-ES-10 | ✅ | `search-field.component.ts` debounced con `debounce(() => timer())` + `distinctUntilChanged`; 0 llamadas HTTP por tecla. ⚠️ Implementado con `debounce()/timer` en vez de `debounceTime` — equivalente, menor. |
| R-ES-11 | ✅ | 12/14 stores con `catchError` → `state.error` + `isLoading:false`; los 2 sin él (`toast`, `settings`) no llaman HTTP. Errores globales en `core/interceptors/error.interceptor`. |
| R-ES-12 | ✅ | `ApiClient` inyectado solo en stores (11 feature + `auth` + `base`) y en `auth.interceptor` (núcleo, R-ES-14); 0 en componentes/páginas. |
| R-ES-13 | ✅ | grep `http://` en `frontend/src` (fuera de environments/specs): 0. |
| R-ES-14 | ✅ | 401/refresh únicamente en `core/interceptors/auth.interceptor.ts`; errores globales en `error.interceptor.ts`. |
| R-ES-15 | ✅ | grep de `inject(...)` en `pages/`, `features/` (fuera de stores), `shared/`, `layout/`: 0 inyecciones fuera de stores + primitivas del framework (`Router`, `ActivatedRoute`, `FormBuilder`, `DestroyRef`, …). |
| R-ES-16 | ✅ | 0 `*.service.ts` y 0 carpetas `services/` en `frontend/src`: la deuda histórica de 4 huérfanos se cerró y su listado se retiró de la regla en esta auditoría. Único servicio = `core/api/api-client.ts`, dependencia solo de stores/interceptor. |
| R-ES-17 | ✅ | Agregaciones en los stores que poseen los datos: `attendance.store` con `withComputed` (`presentCount`, `statusCounts`, `attendanceByWeekday`, `recentRecords`) y método `absentCount(activeCount)`; `employee.store.activeCount`; `device.store.onlineCount`. `dashboard.page` solo ensambla tarjetas, filas de tabla y configuración de gráficas (derivación de vista, R-ES-4) — sin `Math.`/`filter`/`sort`/conteos. Plantillas sin cálculos (grep R-CP-3: 0) ✓. |
| R-ES-18 | ✅ | **2026-09-28 (re-auditoría sobre los 14 stores):** 49 llamadas `console.*` (7 debug, 29 info, 13 error, 0 warn): **49/49 con prefijo `[`** (verificado con parser, no grep de línea) y 0 `console.log` — la estructura `[<ÁREA>:<Origen>] evento → flujo` se cumple (ejemplos: `[AUTH:AuthStore] login OK`, `[REPORT:ReportStore] generateDaily → completado`, `[ATTENDANCE:AttendanceStore] polling → iniciando`, `[DEVICES:DeviceStore] sync → completado`). **Flujos críticos, todos trazados inicio→fin:** login/refresh (`auth.store`, 14 trazas), reportes (`report.store`, 9 + cancelaciones en la página), **polling** (`attendance.store`: `iniciando`/`detenido` + `loadAll → fallo` — D-FE-6) y **sync de dispositivo** (`device.store`: `iniciando`/`completado`/`fallo` — D-FE-6). Los 10 stores CRUD sin trazas no violan la regla (solo los críticos exigen traza) y sus fallos HTTP ya los registra `error.interceptor`. D-FE-4 se mantiene cerrada (grep `console.warn`: 0). |

## Deudas abiertas

Ninguna en la familia `R-ES` (18/18 ✅). Las observaciones menores (implementación
de debounce con `debounce()/timer` en R-ES-10, redacción feature→feature de R-ES-5)
no son deudas: no impiden el cumplimiento.

## Deudas cerradas en esta auditoría

- **D-FE-2 (R-ES-6):** los 3 `.subscribe()` de componentes migraron a `toSignal`
  y `effect` (`search-field`: `controlValue`/`debouncedSearch`; `layout`:
  `navigationEnd`); suscripciones gestionadas por Angular (sin fuga, sin
  `takeUntilDestroyed` que manual). Spec adaptado con flush de effects + 2 tests.
- **D-FE-3 (R-ES-8):** `refreshServerLogs` → `rxMethod` + `mergeMap` y polling en
  tiempo real → `rxMethod` con comando de stop (`null` vía `switchMap`), manteniendo
  el comportamiento (paralelo, cancelación al reiniciar, stop en `onDestroy`).

- **D-FE-1 (R-ES-17):** las agregaciones del dashboard se movieron a `withComputed`
  en los stores (`attendance`: `presentCount`/`statusCounts`/`attendanceByWeekday`/
  `recentRecords` + método `absentCount`; `employee`: `activeCount`; `device`:
  `onlineCount`); `dashboard.page` quedó como ensamblado de presentación.
- **D-FE-4 (R-ES-18):** los 4 `console.warn` migraron — `auth.store.ts` y
  `auth.interceptor.ts:46` → `console.error` (rutas de fallo); `role.guard.ts` y
  `auth.interceptor.ts:40` → `console.info` (eventos de flujo). grep `console.warn`: 0.
- **D-FE-5 (R-ES-18):** el flujo de reportes estaba sin trazas — `generate()` y
  `export()` se cancelaban en silencio con rango inválido y el store no registraba
  ni inicio ni resultado. Ahora la página registra las cancelaciones
  (`[REPORT:ReportGeneratorPage] rango inválido → … cancelado`) y el store traza
  inicio (`info`), resultado (`info` con nº de filas/bytes) y fallo (`error`)
  en `generateDaily`, `generateMonthly` y `exportReport`; el catch global de
  `main.ts` pasó a etiquetarse. R-ES-18 se extendió con el requisito de trazar
  los flujos críticos (ninguna acción muere en silencio).
- **D-FE-6 (R-ES-18, cerrada 2026-09-28):** dos flujos críticos muerían en
  silencio — `attendance.store` (arranque/detención de polling) y `device.store`
  (sincronización) no tenían **ninguna** traza. Fix: 6 trazas nuevas
  (4 `info` de inicio/resultado + 2 `error` de fallo) con la estructura
  `[ATTENDANCE:AttendanceStore] polling → iniciando/detenido`,
  `[ATTENDANCE:AttendanceStore] loadAll → fallo`,
  `[DEVICES:DeviceStore] sync → iniciando/completado/fallo`; el ciclo de vida
  del polling y las trazas de sync quedan asertados en specs (118/118).

- **R-ES-16 (huérfanos):** los 4 servicios listados ya no existen (0 `*.service.ts`
  en `frontend/src`); el listado se retiró del texto de la regla.
- **Conteo del preámbulo de `04`:** "los 11 stores" → **13** signalStores reales
  (auth, toast y 11 de feature); texto corregido.

## Reglas hermanas consultadas

- **R-ES-4 ↔ R-ES-17:** R-ES-4 permite `computed()` en la página (derivación de
  vista); R-ES-17 prohíbe que la página calcule lógica de negocio. Tras D-FE-1,
  el dashboard solo arma su view-model con valores de stores (ambas ✅).
- **R-CP-3:** grep de cálculos en plantillas (`{{ … * | / | - … }}`): 0 — el
  hallazgo del dashboard está en el `.ts`, no en el template.
- **R-FD-6 / R-ES-5:** la prohibición de importar "otra feature" es feature→feature;
  las páginas (capa superior) pueden inyectar varios stores sin violar R-FD-6.
- **R-E-8 (E2E):** complementaria — el runtime lo cubre la suite Playwright
  (cero errores de consola durante el flujo); esta auditoría cubre el código en reposo.
- **Fuera de alcance:** `R-NM`, `R-CP` (salvo R-CP-3), `R-PF`, `R-FR`, `R-FD`
  (salvo R-FD-6) y `R-UX` — se auditarán en la próxima extensión del snapshot.

## Cómo se verificó

`grep`/`find`/`perl` sobre `frontend/src` (consolas y su formato — recuento final
42 = 6 debug / 24 info / 12 error, 42/42 etiquetadas —, `.subscribe()`
fuera de specs, `inject`, imports cruzados, mutaciones directas, `http://`,
`catchError`, `effect`), lectura de los puntos señalados y re-ejecución completa
tras cerrar las 5 deudas: **suite unitaria 77/77** (Karma/ChromeHeadless),
`tsc --noEmit`, `prettier --check` y `ng build` en verde; cobertura **por encima
de `main` en las 4 métricas** — statements 65.47% (vs 64.87), branches 49.74%
(vs 48.97), functions 59.85% (=), lines 65.49% (vs 64.86) — gates 44/32/37/43
incólumes (R-COV-1 ✓).
