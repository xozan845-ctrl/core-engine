# SLO y operación — Core Engine

Objetivo del piloto y de la primera etapa de producción. Los valores se revisan
por trimestre; un cambio de SLO exige ADR.

## 1. SLI/SLO

| SLI | Definición | SLO | Ventana |
|---|---|---|---|
| Disponibilidad | `2xx+3xx+4xx / total` en el gateway | **99,5 %** | 30 días |
| Errores 5xx | `5xx / total` en el gateway | **< 0,5 %** | 30 días |
| Latencia | p95 de `core_engine_http_request_duration_seconds` | **< 1,5 s** | 30 días |
| Outbox | eventos pendientes por servicio | **< 100** | 5 min |
| DLQ | crecimiento neto de `.dlq` | **0** | 15 min |
| Backup | último dump correcto | **< 26 h** | diario |
| Recuperación | restore verificado | **RTO 2 h / RPO 24 h** | por incidente |

El error budget es **0,5 % mensual** (≈ 3 h 36 min de indisponibilidad). Las
alertas de quema están en `infra/prometheus/rules/slo.yml`.

## 2. On-call

- **Primario:** responsable de guardia 24×7 en horario de operación; responde
  en ≤ 15 min a `critical` y ≤ 1 h a `warning`.
- **Secundario:** backend de la plataforma; entra si el primario no responde en
  15 min o si el incidente toca datos/dinero.
- **Escalado:** primario → secundario → responsable de plataforma → CTO.
- **Canal:** el webhook de Alertmanager (`ALERTMANAGER_WEBHOOK_URL`) debe apuntar
  a Slack/PagerDuty/telefono del primario. Sin esa URL, las alertas solo se ven
  en la UI de Alertmanager y **no hay guardia real**.
- **Traspaso:** cada cambio de guardia deja un resumen de alertas abiertas,
  incidentes y despliegues recientes.

## 3. Severidades

| Severidad | Criterio | Respuesta |
|---|---|---|
| **SEV-1** | Caída total, pérdida/corrupción de datos, cobro o facturación incorrecta, fuga cross-tenant | Página inmediata; war room; congelar escrituras si hay riesgo de datos |
| **SEV-2** | Servicio crítico degradado, 5xx > 2 %, DLQ creciendo, backup > 26 h | Página; mitigar en ≤ 1 h |
| **SEV-3** | Servicio no crítico, latencia alta sin impacto, alerta de capacidad | Ticket; atender en horario |
| **SEV-4** | Deuda, warning cosmético | Backlog |

## 4. Runbooks

- Observabilidad y alertas: [`runbook-observabilidad.md`](./runbook-observabilidad.md)
- Entrega y rollback: [`runbook-entrega.md`](./runbook-entrega.md)
- Backup/restore: [`runbook-backup-restore.md`](./runbook-backup-restore.md)
- Incidentes y postmortem: [`runbook-incidentes.md`](./runbook-incidentes.md)
- Gateway/auth: [`runbook-gateway-auth.md`](./runbook-gateway-auth.md)

## 5. Capacidad y carga

- k6: `security/k6/health-smoke.js` (5 VUs/20 s), `order-flow.js` (2 VUs/60 s).
- Antes de subir tráfico: ejecutar `order-flow` con 10–20 VUs en stage y revisar
  p95, 5xx, conexiones Postgres y backlog de outbox.
- Límites por contenedor: 1 CPU / 512 MiB; si un servicio los alcanza de forma
  sostenida, se ajusta el límite con evidencia, no a ciegas.

## 6. Revisión

- Semanal: errores, latencia, outbox, DLQ, backups.
- Mensual: restore drill, revisión de SLO y capacidad.
- Cada incidente SEV-1/SEV-2: postmortem sin culpables en ≤ 5 días.
