# Reglas de Git — Core Engine

Reglas que **todo** cambio versionado debe cumplir antes de llegar a `origin`:
cómo se arma el stage (`git add`), cómo se narra el commit (`git commit -m`),
cómo se publica (`git push`) y **por dónde entra el trabajo** (`gh pr`: rama,
cuerpo, checks y merge). Un historial limpio es infraestructura: los commits
son la documentación viva de por qué existe cada línea.

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `00-entornos.md` | Modelo de ramas: tres entornos (desarrollo/stage/producción), promoción, hotfix y backport | `R-GE` (1–6) |
| `01-stage.md` | `git add`: stage explícito, atómico, sin artefactos ni secretos | `R-GA` (1–5) |
| `02-commits.md` | `git commit`: Conventional Commits, unidad temática, verificación pre-commit | `R-GC` (1–7) |
| `03-push.md` | `git push`: qué subir, cuándo, force-push, divergencia y verificación post-push | `R-GP` (1–7) |
| `04-pr.md` | `gh pr`: rama por trabajo, cuerpo con evidencia, checks en la cabeza, merge `--rebase`, verificación de `main` | `R-PR` (1–9) |

> Los documentos derivados de este área (**CHECKLIST, AUDIT, AUDIT-HISTORY**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-GE-*` | 1–6 | `00-entornos.md` | Entornos y promoción de ramas (`develop` → `staging` → `main`), hotfix |
| `R-GA-*` | 1–5 | `01-stage.md` | Stage (`git add`) |
| `R-GC-*` | 1–7 | `02-commits.md` | Commit (`git commit -m`) |
| `R-GP-*` | 1–7 | `03-push.md` | Push (`git push`) |
| `R-PR-*` | 1–9 | `04-pr.md` | PR (`gh pr`): rama, cuerpo, checks, merge |

## Principio rector

> **Un push es una declaración profesional.** Antes de que un commit llegue a
> `origin` debe poder resumirse en una frase (R-GC-3), haberse armado con stage
> explícito (R-GA-1) y pasar la verificación de su ámbito (R-GC-5). Lo que no se
> puede resumir en una frase, no se sube.
>
> **Y una rama es una unidad revisable.** `develop` no recibe trabajo escrito a
> mano: lo recibe de un PR con evidencia en el cuerpo (R-PR-4), checks verdes en
> su cabeza (R-PR-6) y merge por rebase (R-PR-7). El resultado se comprueba en
> la rama de entorno, no se da por cerrado en el PR (R-PR-8).
>
> **Y un merge a `main` es una promoción.** Producción (R-GE-1) solo recibe el
> salto `staging → main` deliberado con CI verde y smoke (R-GE-2/3, R-CD-11/12);
> nada se escribe a mano en `main` (R-GE-4).

## Convenciones (mantener el "punto fijo")

1. **Formato de ID:** `R-<SUFIXO>-<n>` secuencial e incremental dentro del archivo.
   Un sufijo pertenece a **un solo archivo** (GE, GA, GC, GP). Nunca renumerar ni
   reutilizar IDs existentes.
2. **Formato de regla:** tabla `| ID | Regla |`; redacción en imperativo, concreta
   y verificable en un PR (si no se puede comprobar, no es una regla).
3. **Archivos:** numeración `NN-tema.md` con título `# NN — Tema`. Un archivo nuevo
   = un sufijo nuevo.
4. **Referencias cruzadas:** siempre con el ID completo (`R-GC-5`), jamás "la regla
   5". Antes de renombrar/eliminar una regla, `grep -rn "R-GA-\|R-GC-\|R-GP-" rules/`
   para actualizar todas las referencias (incluye `../_meta/`).
5. **Agregar una regla nueva:** añadirla en su archivo → actualizar el **Mapa de
   IDs** de este README → reflejarla en [`../_meta/CHECKLIST.md`](../_meta/CHECKLIST.md)
   → re-auditar (R-COV-3).
6. **Estado ≠ regla:** las reglas son permanentes; el cumplimiento vive en
   [`../_meta/AUDIT.md`](../_meta/AUDIT.md) (snapshot) y
   [`../_meta/AUDIT-HISTORY.md`](../_meta/AUDIT-HISTORY.md) (log).

## Cómo verificar (comandos)

```bash
# stage revisado: solo los archivos del cambio
git status --short && git diff --cached --stat

# qué se va a subir exactamente (debe coincidir con lo previsto)
git log origin/main..HEAD --oneline

# asuntos de commit conformes a Conventional Commits (debe imprimir vacío)
git log origin/main..HEAD --pretty=%s \
  | grep -vE '^(feat|fix|docs|test|refactor|perf|style|ci|build|chore)(\([a-z-]+\))?: .+'

# verificación del ámbito antes del commit (ej. un servicio)
npm run lint
npm run build
npm run test -w @core/orders-service

# main lineal (R-PR-7): debe imprimir 0
git rev-list --merges --count origin/main

# checks verdes en la CABEZA del PR (R-PR-6) y verificación post-merge (R-PR-8)
gh pr checks <n>
gh run list --branch main --limit 3 --json headSha,conclusion,status
gh run list --branch staging --limit 3 --json headSha,conclusion,status   # R-GE-1/3
```
