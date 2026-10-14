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
| `02-entornos.md` | Etapas (desarrollo local + stage + producción), un proyecto Dockploy con environments, promoción y gate de producción, smoke post-despliegue, rollback | `R-CD` (9–13) |

> El modelo de ramas que estos despliegues consumen vive en
> [`../git/00-entornos.md`](../git/00-entornos.md) (`R-GE-*`): aquí se despliega
> lo que las ramas deciden.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-CD-*` | 1–8 | `01-despliegue.md` | Artefactos de release (tag, digest, OCI, migraciones, secretos) |
| `R-CD-*` | 9–13 | `02-entornos.md` | Entornos Dockploy, promoción, gate, smoke y rollback |

## Guía rápida del despliegue

| Etapa | Rama (R-GE-1) | Dónde corre | Cómo llega |
|---|---|---|---|
| Desarrollo | `develop` | **local** (`docker compose up`) | PR de feature (R-GE-2); sin despliegue |
| Stage | `staging` | Dockploy, environment `staging` | PR `develop → staging` |
| Producción | `main` | Dockploy, environment `produccion` | PR `staging → main` (R-GE-4) |

## Pendientes y estado

- **`release.yml` (2026-10-14)**: existe el release por tag `vX.Y.Z` con
  procedencia (commit en `main` + `ci.yml` verde en el mismo SHA, `R-CD-2`),
  publicación de **una imagen por servicio** en GHCR con etiquetas OCI/semver
  (`R-CD-4`) y digest en el resumen. `promote.yml` las mueve a `stage`/`prod`
  **por digest** (`R-CD-3`) y `deploy.yml` valida el arranque (`R-CD-12`). Los
  composes `docker-compose.stage.yml` / `docker-compose.prod.yml` consumen GHCR.
  Dockploy sigue siendo el destino del VPS (`R-CD-9..13`). Ver el runbook
  [`../../runbook-entrega.md`](../../runbook-entrega.md).
- **Dockploy se configura fuera de este repo**: un solo proyecto con los
  environments `staging`/`produccion` (R-CD-9) y la política de producción
  (R-CD-10/11) se documentan aquí; la configuración concreta del VPS queda
  registrada en `docs/dockploy-setup.md` (Etapa D3).
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