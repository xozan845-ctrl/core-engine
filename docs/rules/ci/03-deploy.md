# 03 — Release y despliegue

Aplica a: versionado por tag, construcción y promoción de imágenes, y qué
significa desplegar una release (`.github/workflows/release.yml`).

> **Estado: N/A hoy.** Este repo no tiene `release.yml` (solo existe
> [`ci.yml`](../../../.github/workflows/ci.yml), jobs `security-gate` → `lint`/`test` →
> `build`); las reglas de abajo son el contrato exigible **el día que se cree
> ese workflow** (R-CI-4), no la descripción de algo que existe. Hasta
> entonces no se auditan como incumplimiento.

| ID | Regla |
|---|---|
| R-CD-1 | **Versionado inmutable por tag**: una release se dispara exclusivamente al hacer `push` de un tag `v<major>.<minor>.<patch>`, con sufijo prerelease opcional (`-alpha.1`, `-rc.2`); el patrón se valida antes de construir y un tag inválido aborta el run. Nunca desde una rama, nunca reconstruyendo `main`. Un tag publicado no se mueve ni se borra: un error se corrige con un tag nuevo, igual que una release publicada no se reescribe. |
| R-CD-2 | **Procedencia verificada antes de publicar**: el commit etiquetado tiene que estar en `main` (`git merge-base --is-ancestor`) y tener al menos un run de `ci.yml` con `conclusion == success` en **ese mismo SHA**; si no hay ninguno, el run aborta y lista los runs que sí hubo para el diagnóstico. R-GP-6 ya prohíbe dar por bueno el verde de otro commit; aquí es condición de publicación, no una recomendación. `workflow_dispatch` sirve para repetir el proceso de un tag existente, nunca para publicar código que no tiene CI verde. |
| R-CD-3 | **Construir una vez, promover por digest**: las imágenes se construyen una sola vez y de ahí en adelante se promueve por `@sha256:`, sin reconstruirlas. Un digest es inmutable y un tag de Docker es una etiqueta móvil: promover por tag rompe la reproducibilidad en cuanto algo vuelve a publicarse con el mismo nombre. Si hay que reconstruir, es otro tag y otro digest. |
| R-CD-4 | **Trazabilidad**: cada imagen lleva etiquetas OCI con el origen (`org.opencontainers.image.revision` = SHA del commit, `source` = URL del repo) y se nombra con versión semver (`1.2.3`, `1.2`, `1`), `sha-<corto>` para identificar el commit sin depender de semver, y `latest` **solo** en releases no-prerelease. `latest` nunca apunta a una alpha ni a una rc. El digest de lo publicado queda en el resumen del run (`$GITHUB_STEP_SUMMARY`) de cada job de imagen. |
| R-CD-5 | **Migraciones solo hacia adelante**: desplegar aplica el esquema **solo desde el DDL versionado en `infra/db/init/*.sql`** (R-DB-6), nunca una mutación ad-hoc de esquema (misma razón que R-EN-2 en CI), y el rollback de una release es **volver a desplegar el digest anterior**, nunca revertir el historial de migraciones: deshacer migraciones ya aplicadas en producción significa perder datos, y ninguna reversión de SQL es segura por defecto. Si una migración no es reversible, se documenta como tal en su commit y su plan de rollback es el digest anterior. |
| R-CD-6 | **Producción exige aprobación, y hoy no hay a dónde desplegar**: el día que exista despliegue a una máquina, este va contra un `environment` con revisores obligatorios, de modo que ningún push llega solo a producción, y ningún runner self-hosted recibe secretos de producción. Mientras tanto —no hay host definido, ni credenciales de acceso, y el único compose del repo (`docker-compose.yml`) construye desde fuente (`build:`)— el workflow **publica imágenes y release pero no despliega en ninguna máquina**, y no debe hacerlo hasta que esa regla sea exigible. Publicar un artefacto es reversible; ejecutar migraciones sobre datos reales no lo es. |
| R-CD-7 | **Cero secretos en la imagen**: ninguna credencial entra como `ARG` ni como `ENV` de construcción, y ninguna queda en una capa. Los secretos se inyectan en tiempo de ejecución. Un secreto en una capa es un secreto público para quien pueda descargar la imagen, y las capas no se borran: se añaden. |
| R-CD-8 | **Simulacro por defecto**: `workflow_dispatch` trae `dry_run: true` como valor por defecto, así que una ejecución manual construye las imágenes, las descarta y no publica nada. Publicar exige un tag, o bien `dry_run: false` explícito en la ejecución manual. Es la diferencia entre probar el proceso y cambiar el estado del mundo. |

## Qué no cubre este archivo

El despliegue a una máquina concreta **todavía no está definido**: no hay host,
ni usuario de despliegue, ni credenciales de acceso, y el único compose del
repo (`docker-compose.yml`) construye desde fuente (`build:`) en lugar de
consumir imágenes de un registro.
Decidir el modelo de distribución —registro y `image:` en el compose, frente a
`docker compose build` en el servidor— es una decisión de arquitectura, no un
detalle de workflow, y no se toma a la sombra de un fichero YAML.

Mientras tanto R-CD-6 deja constancia de lo que sí es exigible cuando exista: el
cambio de `docker-compose.yml` a `image:` con digest, y la primera vez que
se aplique una migración en un entorno real, son cambios de regla.