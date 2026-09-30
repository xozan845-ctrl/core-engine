# 01 — Flujos de CI (gatillos y ejecución)

Aplica a: cuándo y cómo se ejecutan los jobs de integración
(`.github/workflows/ci.yml`).

| ID | Regla |
|---|---|
| R-CI-1 | **Gatillos**: todo push a `main` corre la CI completa (jobs `backend`, `frontend`, `e2e`, `playwright`); un PR hacia `main` corre todos los jobs **excepto** los de release (`playwright`/G-6, marcado `if: github.event_name == 'push'`); un push a rama sin PR no ejecuta nada. Cambiar este reparto exige enmienda explícita de esta regla. |
| R-CI-2 | **Concurrencia**: cada workflow declara un grupo por ref con `cancel-in-progress: true` — un push nuevo cancela el run anterior de la misma rama; prohibido apilar runs obsoletos. |
| R-CI-3 | **Sin filtros `paths`**: todo evento push/PR ejecuta la CI completa, aunque el cambio sea solo de docs (decisión deliberada: nada pasa desapercibido). Introducir filtrado = enmienda de esta regla, nunca una optimización silenciosa. |
| R-CI-4 | **Pipeline único**: `.github/workflows/ci.yml` es el único pipeline de integración. Un workflow adicional solo se acepta con un caso de uso que este archivo no cubra, documentado en este área antes de existir. |
| R-CI-5 | Los cambios en `.github/workflows/**` se revisan como código: stage con rutas explícitas (R-GA-1), sin secretos (R-GA-4), comandos idénticos a los verificables en local, y el mensaje de commit cita las reglas tocadas (R-GC-4). |
| R-CI-6 | **Verificación con GitHub CLI (`gh`)**: tras cada push, el resultado se comprueba en local con `gh run list --limit 5` y `gh run view <id>` (detalle por job); mientras corre, `gh run watch <id>`; si falla, `gh run view <id> --log-failed` para leer el log del job rojo. Un trabajo no se da por terminado hasta ver `completed success` en el run del commit — el verde de un run anterior no vale nada para el actual (R-GP-6). Run en rojo ⇒ fix-forward con un commit nuevo que cite el fallo (R-GP-3); nunca se deja la rama en roja sin diagnosticar. Requiere `gh auth login` una vez por máquina. |
