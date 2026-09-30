# Reglas de CI — Core Engine

Mecánica de la integración continua: cuándo se ejecuta qué, concurrencia,
entorno del runner y evidencia. **Los gates de calidad (G-1…G-8) no viven aquí**:
son reglas de testing y siguen en
[`../test/06-estandares-cobertura.md`](../test/06-estandares-cobertura.md).

## Archivos

| Archivo | Contenido | IDs |
|---|---|---|
| `01-flujos.md` | Gatillos push/PR, concurrencia, sin `paths`, pipeline único, revisión de `.github/`, verificación de runs con `gh` | `R-CI` (1–6) |
| `02-entorno.md` | Runner/Node/pnpm, BD efímera, artifacts de evidencia, secretos en CI | `R-EN` (1–4) |
| `03-deploy.md` | Release por tag, procedencia y CI verde, imágenes por digest, migraciones en despliegue, aprobación de producción, secretos en la imagen | `R-CD` (1–8) |

> Área conforme a la regla de estructura: solo `README.md` + archivos `NN-tema.md`.
> Los derivados (AUDIT, CHECKLIST, historiales) viven en [`../_meta/`](../_meta/README.md).

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-CI-*` | 1–6 | `01-flujos.md` | Flujos, ejecución y verificación de CI |
| `R-EN-*` | 1–4 | `02-entorno.md` | Entorno y evidencia en CI |
| `R-CD-*` | 1–8 | `03-deploy.md` | Versionado, imágenes y despliegue |

## Reglas de otras áreas que exige CI (cross-refs)

| ID | Exigencia en CI |
|---|---|
| G-1…G-8 | Gates de testing que los jobs deben dejar en verde ([`../test/06`](../test/06-estandares-cobertura.md)) |
| R-E-11/12 | Suite E2E hermética; jobs con `TZ: America/Managua`, retry único y timeout por paso |
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
| `gh release list` | ¿Hay ya una release para este tag? Un tag publicado no se mueve (R-CD-1) |
| `gh run list --workflow release.yml` | Estado del run de la release: construido, publicado o abortado por procedencia (R-CD-2) |

La importancia es cerrar la evidencia: un cambio "terminado" es un commit con
`completed success` en **su** run (R-CI-6). Todo lo demás es opinión.

## Pendientes

- **Workflow de release aún no existe aquí**: las reglas `R-CD` de
  [`03-deploy.md`](./03-deploy.md) se heredaron junto con el `release.yml` y el
  `docker-compose.prod.yml` del proyecto origen; en este repo solo existe
  [`ci.yml`](../../../.github/workflows/ci.yml) (jobs `security-gate`, `test`,
  `build`). Definir el flujo de release de Core Engine —tag, build, imágenes,
  despliegue— es una decisión de arquitectura y un cambio de regla.
- **Contenido heredado a auditar**: `01-flujos.md` (jobs `backend`, `frontend`,
  `e2e`, `playwright`), `02-entorno.md` (pnpm, Prisma, BD efímera por job) y
  `03-deploy.md` describen el pipeline del proyecto origen, no el `ci.yml`
  actual. Re-auditarlos contra el pipeline real es trabajo de la próxima
  auditoría (R-COV-3).
