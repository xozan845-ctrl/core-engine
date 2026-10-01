# 03 — Push: `git push`

## Reglas

| ID | Regla |
|---|---|
| R-GP-1 | Antes de pushear: `git status` limpio (0 cambios sin commitear) y `git log origin/main..HEAD --oneline` leído commit por commit: si la lista no coincide exactamente con lo que se quiere subir, no pushear. |
| R-GP-2 | Subir solo con la verificación local en verde (R-GC-5) y, si el repo tiene CI, con los checks del último commit en verde; un push con CI roja se corrige con commit nuevo, nunca con force. |
| R-GP-3 | Prohibido `git push --force` y `--force-with-lease` sobre `main`, `staging`, `develop` (ramas de entorno, R-GE-1) o cualquier rama compartida: el historial publicado no se reescribe (coherencia con R-GC-7). |
| R-GP-4 | `origin` solo recibe trabajo terminado y verificado: el WIP vive en el árbol local sin commitear o en una rama local nunca publicada; prohibido subir "respaldos" (`chore: wip`). |
| R-GP-5 | Si `origin/main` avanzó respecto a local: `git pull --rebase` antes del push (historial lineal); prohibido el merge de sincronización y prohibido resolverlo con force. |
| R-GP-6 | Tras el push: `git status -sb` debe reportar `main...origin/main` al día y el CI del commit debe pasar, **verificado con `gh run list` / `gh run view`** (R-CI-6) — el verde de un run anterior no prueba nada sobre el actual; si queda en roja, diagnosticar con `gh run view <id> --log-failed` y fix-forward con un commit nuevo (R-GP-3). |
| R-GP-7 | Verificar que el push contiene SOLO los commits propios (`git log origin/main..HEAD`): nunca subir commits ajenos arrastrados por un pull o rebase mal resuelto. |

## Verificación

```bash
git status -sb                        # main...origin/main al día tras el push
git log origin/main..HEAD --oneline    # vacío tras el push; antes = lo previsto
git log origin/main --oneline -5       # contexto del historial publicado

gh run list --limit 5                 # ¿el run del push corrió y en verde? (R-CI-6)
gh run view <id>                      # jobs del run; --log-failed = log del fallo
```
