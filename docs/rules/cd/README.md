# Reglas de CD (Continuous Deployment) — Core Engine

Despliegue continuo sobre el VPS con **Dockploy**: cómo se entregan los tres
entornos (desarrollo → stage → producción), qué gate cruza cada cambio antes de
tocar producción y cómo se revierte un despliegue malo.

**`R-CD-1..8`** vinieron de `ci/03-deploy.md` el 2026-10-01 al crearse esta área
(mismas IDs, nunca se renumeran); **`R-CD-9..13`** son las reglas nuevas de
entornos y Dockploy.

## Archivos

| Archivo | Contenido | IDs |
|---|---|---|
| `01-despliegue.md` | Release por tag, procedencia y CI verde, imágenes por digest, migraciones solo hacia adelante, aprobación de producción, secretos en la imagen, dry-run | `R-CD` (1–8) |
| `02-entornos.md` | Tres entornos en Dockploy (apps por rama), promoción y gate de producción, smoke post-despliegue, rollback | `R-CD` (9–13) |

> El modelo de ramas que estos despliegues consumen vive en
> [`../git/00-entornos.md`](../git/00-entornos.md) (`R-GE-*`): aquí se despliega
> lo que las ramas deciden.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-CD-*` | 1–8 | `01-despliegue.md` | Artefactos de release (tag, digest, OCI, migraciones, secretos) |
| `R-CD-*` | 9–13 | `02-entornos.md` | Entornos Dockploy, promoción, gate, smoke y rollback |

## Guía rápida del despliegue

| Entorno | Rama (R-GE-1) | Deploy | Cómo llega |
|---|---|---|---|
| Desarrollo | `develop` | Dockploy app dev (auto) | PR de feature (R-GE-2) |
| Stage | `staging` | Dockploy app stage (auto) | PR `develop → staging` |
| Producción | `main` | Dockploy app prod (auto) | PR `staging → main` (R-GE-4) |

## Pendientes y estado

- **`release.yml` no existe**: no hay release por tag ni publicación de imágenes
  a un registry; el VPS construye desde el `Dockerfile` raíz con cada push a
  `main` (Dockploy). Los `R-CD-1..8` son el contrato exigible el día que exista
  un workflow de release (R-CI-4); los `R-CD-9..13` describen el despliegue que
  **sí** existe hoy.
- **Dockploy se configura fuera de este repo**: las apps por entorno (R-CD-9) y
  la política de producción (R-CD-10/11) se documentan aquí; la configuración
  concreta del VPS (nombres de app, BD por entorno) no se versiona y queda
  registrada en la Etapa D3 del modelo.
- **Traslado**: con la creación de esta área, `ci/` queda solo con integración
  (`R-CI`/`R-EN`); ver [`../ci/README.md`](../ci/README.md).

## Cómo verificar

```bash
# runs por rama de entorno (R-GE-1, R-PR-8): los saltos dejan CI verde en destino
gh run list --branch develop --limit 3
gh run list --branch staging --limit 3
gh run list --branch main --limit 3

# sanidad del deploy de producción (smoke, R-CD-12)
curl -fsS http://<gateway-prod>/health
```