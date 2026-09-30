# Reglas de Gateway — Core Engine

Reglas doradas del **API Gateway** (`packages/api-gateway`, :8080): único
punto de entrada, descarga perimetral (CORS/throttling/TTL), validación JWT
con prevención de spoofing, TTLs de token, errores RFC 7807 y versionamiento
de rutas (`/api/v1/`).

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `01-auth.md` | Single entry point, edge offloading, JWT + scrub de headers, TTLs, errores, versionamiento | `R-GW` (1–7) |

> Los documentos derivados de este área (**AUDIT, CHECKLIST, historiales**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-GW-*` | 1–7 | `01-auth.md` | Perímetro, autenticación y contrato externo |

## Reglas de otras áreas que exige esta (cross-refs)

| ID | Exigencia |
|---|---|
| `R-GW-2` | Rate limiting del borde → `R-DS-6` (`../data/02-seguridad.md`) |
| `R-GW-3` | Scrub de `x-internal-key` → `R-DS-5` (`../data/02-seguridad.md`) |
| `R-GW-4` | TTLs por env, nunca hardcodeados → `R-DS-1` (`../data/02-seguridad.md`) |
| `R-GW-7` | Prefijo `/api/v1/` en todas las rutas (ver `README.md` raíz del repo) |

## Pendientes

- Pendiente de auditar: entra en la próxima revisión (`R-COV-3`).
