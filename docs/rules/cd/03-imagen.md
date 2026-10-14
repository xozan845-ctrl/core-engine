# 03 — Higiene de la imagen (R-CD-14)

Reglas de la **imagen de runtime**: usuario no-root y poda de las dependencias de
desarrollo. Complementa el contrato de imágenes de
[`01-despliegue.md`](./01-despliegue.md) (`R-CD-1..8`).

| ID | Regla |
|---|---|
| R-CD-14 | **Higiene de la imagen de runtime**: cada imagen corre como usuario no-root (`USER node`) y poda las devDependencies tras compilar (`npm prune --omit=dev`), de modo que el tooling de test y build (vitest, swc, eslint…) no viaja en la imagen publicada. Aplica al `Dockerfile` raíz, a `packages/Dockerfile.template` y a cada `packages/<servicio>/Dockerfile`. |
