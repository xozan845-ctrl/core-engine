# 02 — Commit: `git commit -m`

## Reglas

| ID | Regla |
|---|---|
| R-GC-1 | Mensaje en Conventional Commits: `<tipo>(<ámbito>): <resumen en imperativo, español>`. Tipos admitidos: `feat, fix, docs, test, refactor, perf, style, ci, build, chore`; ámbitos reales del cambio (`gateway`, `orders`, `shared`, `rules`, `ci`, …). Ejemplo: `refactor(orders): validar transicion de estado (R-DI-3)`. |
| R-GC-2 | El asunto dice QUÉ cambia y POR QUÉ importa, en una sola línea; prohibidos `wip`, `fix`, `update`, `changes`, `add stuff` y cualquier mensaje que solo repita el nombre del archivo tocado. |
| R-GC-3 | Un commit = una unidad temática que puede resumirse en una frase; prohibido mezclar feature + fix + formato + dependencias en el mismo commit. |
| R-GC-4 | Si el cambio implementa, corrige o verifica una regla, citar su ID completo al final del asunto o en el cuerpo (`(R-DI-3)`); si el cambio viene de una deuda de auditoría, citar el ítem correspondiente. |
| R-GC-5 | Dejar en verde la verificación del ámbito tocado ANTES de commitear (`npm run lint`, `npm run build` y la suite del servicio: `npm run test -w @core/orders-service`); commitear roto está prohibido: el código roto se arregla o se queda fuera del commit. |
| R-GC-6 | Cuando el asunto no alcanza a explicar el porqué, usar el cuerpo del commit: bug (síntoma + causa), refactor (motivo), cambio rompente (cómo migrar). Formato: `git commit -m "asunto" -m "cuerpo"`. |
| R-GC-7 | No reescribir commits ya publicados: `--amend` o rebase solo sobre commits locales que NUNCA subieron a `origin`; sobre `origin` las correcciones van como commit nuevo. |

## Verificación

```bash
# asuntos conformes a Conventional Commits (debe imprimir vacío)
git log origin/main..HEAD --pretty=%s \
  | grep -vE '^(feat|fix|docs|test|refactor|perf|style|ci|build|chore)(\([a-z-]+\))?: .+'

# unidad temática: qué toca el commit
git show --stat HEAD
```
