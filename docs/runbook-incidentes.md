# Runbook — incidentes y postmortem

## 1. Primeros 5 minutos

1. **Declarar** el incidente en el canal de guardia con severidad (SEV-1..4) y
   hora.
2. **Congelar** despliegues y migraciones si hay riesgo de datos o dinero.
3. **Identificar** el servicio y la ruta con el `x-request-id` de una petición
   fallida; revisar `/metrics`, logs JSON y estado de contenedores.
4. **Mitigar** antes de diagnosticar: rollback al digest anterior, escalar
   réplicas, pausar un job o cerrar una capacidad.
5. **Comunicar** cada 30 min a negocio: impacto, mitigación y siguiente
   actualización.

## 2. Árbol de decisión

| Síntoma | Primera acción |
|---|---|
| 5xx en un servicio | `/metrics` → logs con `request_id` → rollback del digest si empezó tras un deploy |
| Gateway 502/504 | `/ready` y `docker compose ps`; revisar upstream y timeouts |
| Postgres lento o sin conexiones | conexiones activas, locks, queries largas; bajar tráfico o reiniciar pool |
| Outbox creciendo | revisar RabbitMQ, confirmaciones y DLQ; **no** borrar filas |
| DLQ creciendo | inspeccionar `x-error`, `x-intentos`; corregir causa antes de reinyectar |
| Backup > 26 h | revisar sidecar/credenciales; ejecutar backup y restore drill |
| Cobro/facturación incorrecta | SEV-1: congelar checkout, identificar pagos afectados, no reembolsar a ciegas |
| Fuga cross-tenant | SEV-1: revocar sesiones/API keys, aislar tenant, preservar evidencia |

## 3. Rollback

1. Código: re-promover el digest anterior (`promote.yml` con el tag previo) y
   redesplegar el digest; nunca revertir migraciones.
2. Datos: restaurar el último dump verificado en una base nueva y cambiar
   `DATABASE_URL` en ventana controlada.
3. Capacidad: desactivar una capacidad o un pack por API antes de tocar código.

## 4. Postmortem (≤ 5 días)

- Línea de tiempo con hora, actor, acción y evidencia (`x-request-id`, run ID).
- Impacto: usuarios/tenants, duración, dinero, datos.
- Causa raíz y factores contribuyentes (sin culpables).
- Acciones correctivas con dueño y fecha; al menos una prueba de regresión que
  falle con la causa.
- Actualizar este runbook, el SLO o las alertas si el incidente lo justifica.

## 5. Contactos

- Primario/secundario: se mantienen en el gestor de guardia, no en el repo.
- Escalado técnico: responsable de plataforma y CTO.
- Proveedor de pagos/fiscal: contrato y canal de soporte en el gestor de
  proveedores.
