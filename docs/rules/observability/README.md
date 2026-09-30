# Reglas de Observability — Core Engine

Reglas doradas de **observabilidad**, en dos documentos complementarios que se
aplican juntos: logs estructurados + correlación de trazas (`01-logs.md`) y
métricas Prometheus + dashboards/alertas de Grafana (`02-telemetria.md`).

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `01-logs.md` | Logging JSON, `x-request-id` (síncrono y vía outbox), métricas de negocio, niveles, sanitización | `R-OB` (1–5) |
| `02-telemetria.md` | `/metrics` universal, naming, labels de baja cardinalidad, RED, backlog DLQ, dashboards as code, alertas | `R-TM` (1–7) |

> Los documentos derivados de este área (**AUDIT, CHECKLIST, historiales**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-OB-*` | 1–5 | `01-logs.md` | Logs estructurados y trazas |
| `R-TM-*` | 1–7 | `02-telemetria.md` | Métricas, dashboards y alertas |

## Reglas de otras áreas que exige esta (cross-refs)

| ID | Exigencia |
|---|---|
| `R-OB-2` | El `x-request-id` vive en los metadatos del outbox → `R-AR-3` (`../architecture/01-arquitectura.md`) |
| `R-OB-3` | Nombres de métricas ASCII → `R-TM-2` (mismo área) |
| `R-OB-5` | Sin PII en logs → `R-DS-2` (`../data/02-seguridad.md`) |
| `R-TM-5` | Alerta de DLQ por crecimiento, no por existencia → `R-AR-4` (`../architecture/01-arquitectura.md`) |
| `R-TM-6` | Grafana provisionado desde `infra/grafana/` (ver `README.md` raíz del repo) |

## Pendientes

- Pendiente de auditar: entra en la próxima revisión (`R-COV-3`).
