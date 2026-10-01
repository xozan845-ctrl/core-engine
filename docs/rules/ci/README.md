# Reglas de CI — Core Engine

Mecánica de la integración continua: cuándo se ejecuta qué, concurrencia,
entorno del runner y evidencia. **Los gates de calidad (G-1…G-8) no viven aquí**:
son reglas de testing y siguen en
[`../test/06-estandares-cobertura.md`](../test/06-estandares-cobertura.md).

## Archivos

| Archivo | Contenido | IDs |
|---|---|---|
| `01-flujos.md` | Gatillos push/PR, concurrencia, sin `paths`, pipeline único, revisión de `.github/`, verificación de runs con `gh` | `R-CI` (1–6) |
| `02-entorno.md` | Runner/Node/npm, evidencia (artifacts), BD efímera exigible, secretos en CI | `R-EN` (1–4) |

> El **despliegue** ya no vive en esta área: `R-CD-*` (1–13) se trasladaron a
> [`../cd/`](../cd/README.md) el 2026-10-01 (mismas IDs). CI integra; CD despliega.

> Área conforme a la regla de estructura: solo `README.md` + archivos `NN-tema.md`.
> Los derivados (AUDIT, CHECKLIST, historiales) viven en [`../_meta/`](../_meta/README.md).

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-CI-*` | 1–6 | `01-flujos.md` | Flujos, ejecución y verificación de CI |
| `R-EN-*` | 1–4 | `02-entorno.md` | Entorno y evidencia en CI |

> `R-CD-*` ya no se mapea aquí: ver el [Mapa de IDs](../cd/README.md) de `cd/`.

## Reglas de otras áreas que exige CI (cross-refs)

| ID | Exigencia en CI |
|---|---|
| G-1…G-8 | Gates de testing (estado por gate ✅/⏸/N/A en [`../test/06`](../test/06-estandares-cobertura.md)) |
| R-E-11/12 | Suite E2E hermética; exigible a los jobs E2E cuando existan (`TZ: America/Managua`, retry único y timeout por paso) |
| R-I-12 | Suites ejecutables con BD efímera |
| R-C-7 / G-8 | Spec OpenAPI sincronizado (`git diff --exit-code`) |
| R-FL-1 | Tests intermitentes → quarantine con fecha |
| R-COV-3 | Un gate ✅ solo con run verde de CI |
| R-GP-2/6 | Push con checks verdes; rojo ⇒ fix-forward con commit nuevo (verificación con `gh`, R-CI-6) |

## GitHub CLI (`gh`) — uso e importancia

`gh` es la consola de GitHub y la **única forma verificable desde terminal** de
ver el estado real de la CI (R-CI-6): sin él, cerrar el ciclo *push → run verde*
(R-GP-2/6, R-COV-3) depende de abrir el navegador o de suponer que "el anterior
estaba verde" — que es exactamente lo que la regla prohíbe.

| Comando | Cuándo y qué aporta |
|---|---|
| `gh run list --limit 5` | ¿El push ya corrió?, ¿en verde o rojo?, edad del run |
| `gh run view <id>` | Jobs concretos: qué gate se rompió + anotaciones del runner |
| `gh run watch <id>` | Seguir **en vivo** un run en curso (no sirve si ya terminó) |
| `gh run view <id> --log-failed` | Log del job fallido del run → punto de partida del fix-forward (R-GP-3) |
| `gh auth status` / `gh auth login` | Estado / autenticación (una vez por máquina) |
| `gh release list` | ¿Hay ya una release para este tag? Un tag publicado no se mueve (R-CD-1 en [`../cd/01-despliegue.md`](../cd/01-despliegue.md)) |
| `gh run list --workflow ci.yml` | Estado de la integración: los 4 jobs (`security-gate` → `lint`/`test` → `build`) del único workflow (R-CI-4) |
| `gh run list --workflow release.yml` | Estado del run de la release **(cuando exista `release.yml`, hoy no)**: construido, publicado o abortado por procedencia (R-CD-2 en `cd/01`) |
| `gh run list --branch staging` | CI en la rama de stage (R-GE-1): el run que una promoción `develop → staging` debe dejar verde antes de `staging → main` |

La importancia es cerrar la evidencia: un cambio "terminado" es un commit con
`completed success` en **su** run (R-CI-6). Todo lo demás es opinión.

## Pendientes

- **Workflow de release aún no existe aquí**: en este repo solo existe
  [`ci.yml`](../../../.github/workflows/ci.yml) (jobs `security-gate` → `lint`/`test` →
  `build`), así que los `R-CD-1..8` (en [`../cd/01-despliegue.md`](../cd/01-despliegue.md))
  son el contrato exigible el día que exista un `release.yml` (R-CI-4), no la
  descripción de algo existente; los `R-CD-9..13` (`cd/02`) describen el
  despliegue que sí existe (Dockploy). Definir el flujo de release de Core Engine
  —tag, build, imágenes— es una decisión de arquitectura y un cambio de regla.
- **`ci.yml` sin grupo de concurrencia**: R-CI-2 exige
  `concurrency`/`cancel-in-progress` por ref y el workflow no lo declara, así
  que pushes seguidos en la misma rama apilan runs en paralelo. Deuda de
  configuración (no de redacción), recogida para la auditoría (R-COV-3).
- **`01`/`02` alineados con el pipeline real** (2026-10-01): sus descripciones
  ya coinciden con `ci.yml` (jobs, gatillos con `staging` tras la enmienda de
  R-CI-1, comandos, rutas). Cualquier referencia restante al pipeline heredado
  del proyecto origen fuera de esta área se audita con R-COV-3.
