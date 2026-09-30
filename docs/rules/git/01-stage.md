# 01 — Stage: `git add`

## Reglas

| ID | Regla |
|---|---|
| R-GA-1 | Armar el stage con rutas explícitas (`git add packages/orders-service/src/pedidos/…`); prohibido `git add .`, `git add -A` y `git commit -a` como método de trabajo: el stage se compone a mano, no se barre. |
| R-GA-2 | Antes de commitear, revisar el stage con `git status --short` y `git diff --cached --stat`: si aparece un archivo que no pertenece al cambio, sacarlo con `git restore --staged <ruta>` antes de confirmar. |
| R-GA-3 | Stage atómico: el stage contiene exactamente los archivos que narra el commit (un cambio = un stage = un commit); el trabajo incompleto o no relacionado queda fuera, ni "por si acaso". |
| R-GA-4 | Jamás stagear artefactos ni secretos: `node_modules/`, `dist/`, `coverage/`, `*.log`, `.env*`, claves, tokens, dumps de BD. Si `git status` los muestra, corregir `.gitignore` ANTES de stagear: un secreto versionado no se "arregla después", se reescribe la historia. |
| R-GA-5 | Prohibido `git add -f` sobre archivos ignorados; solo se admite si el archivo es fuente que debe versionarse y la excepción queda justificada en la descripción del PR. |

## Verificación

```bash
git status --short              # nada ajeno al cambio
git diff --cached --stat        # el listado coincide con el commit planeado
git diff --cached | grep -iE "api[_-]?key|secret|password|token"   # vacío
```
