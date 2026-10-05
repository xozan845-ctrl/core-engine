# 01 — Contribuciones de IA

El objetivo de esta área es asegurar que los agentes de inteligencia artificial no diluyan, ignoren ni rompan la fuerte cultura de diseño por contrato y reglas de Core Engine. Las IAs son un amplificador, y un amplificador no debe amplificar entropía.

## Reglas

| ID | Regla |
|---|---|
| R-IA-1 | **Lectura de contexto obligatoria:** Antes de proponer o aplicar cambios, la IA debe mapear el estado actual leyendo `docs/rules/README.md`. Si tocará código, debe leer las áreas relevantes (ej. `rules/test/`, `rules/data/`) y apoyarse en `_meta/AUDIT.md` para conocer las deudas abiertas. |
| R-IA-2 | **Uso adecuado de herramientas (Tooling):** La IA debe respetar sus `CRITICAL INSTRUCTIONS`. Debe usar comandos especializados (`grep_search`, `replace_file_content`) en lugar de `cat`, `sed` o `grep` empaquetados en scripts de shell. Prohibido manipular archivos sin las herramientas nativas. |
| R-IA-3 | **Convenciones atómicas y Git estricto:** La IA está sujeta al mismo estándar que los humanos (`rules/git/`). Cada hallazgo u objetivo es un commit atómico. Sus mensajes siguen Conventional Commits en español e incluyen el ID de regla (`fix(auth): corregir hash en JWT (R-GW-4)`). Nada de "arreglé algunas cosas". |
| R-IA-4 | **Sin pases libres:** Todo código emitido por IA se somete a los gates (G-1..G-8). Debe proveer tests que aserten contra el requisito real (R0) y no contra su propia implementación. La cobertura no puede bajar (`R-COV-1`). Si inyecta logs, debe usar estructuración JSON (`R-OB-1`). |
| R-IA-5 | **PRs como entregables finales:** La IA no debe comitear a ramas principales (`main`, `develop`, `staging`). Su trabajo se consolida en una rama semántica y se entrega mediante PR. El cuerpo del PR debe contener el `CHECKLIST.md` cumplimentado y tablas reales de evidencia (comandos corridos y sus salidas). |
| R-IA-6 | **Refactorizaciones consensuadas:** Si la IA detecta una "mejora estructural" ajena al task en curso, debe registrarla como deuda técnica. Prohibidos los refactors silenciosos o "oportunistas" no solicitados explícitamente por el usuario que ensucian el PR. |

## Verificación

- El historial de la rama originada por la IA debe reflejar un delta atómico con `git log --oneline`.
- En la revisión, la tabla de "Reglas consultadas" en el PR por parte de la IA debe estar presente.
- La ejecución de `npm run lint` y `npm run build` es exigible en todos sus PRs (G-7).
