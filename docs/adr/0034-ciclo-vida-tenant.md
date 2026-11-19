# ADR-0034 — Ciclo de vida del tenant y bloqueo de sesiones

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado; exportación/borrado en ADR-0035)
- **Relacionado:** [ADR-0017](./0017-identity-service-en-rust.md),
  [ADR-0021](./0021-negocios-por-empresa.md)

## Contexto

`identity.empresas.estado` existía con `activa`, `suspendida` e `inactiva`, pero
no había API para cambiarlo y el login no lo comprobaba. Una empresa suspendida
podía seguir iniciando sesión y usando API keys. Tampoco había un cierre
operativo explícito ni un registro de auditoría del cambio.

## Decisión

1. **API de ciclo de vida.** `PATCH /api/v2/empresas/{id}/estado` con
   `{estado, motivo?}`. Solo `super_admin`/`admin`.
2. **Transiciones.** `activa → suspendida|inactiva`,
   `suspendida → activa|inactiva`, `inactiva → activa`. `inactiva` es el cierre
   operativo; la reactivación queda para un administrador de plataforma.
3. **Auditoría.** Cada cambio inserta `empresa.estado` en
   `identity.auditoria` con estado anterior, nuevo y motivo.
4. **Bloqueo de sesiones.** Suspender o cerrar revoca todas las sesiones
   activas de los usuarios del tenant en la misma transacción.
5. **Bloqueo de autenticación.** Login, refresh, cambio de negocio y aceptación
   de invitación validan `empresas.estado`; si no está `activa` responden
   `403 EMPRESA_SUSPENDIDA` o `403 EMPRESA_INACTIVA`.
6. **API keys.** `POST /auth/token` rechaza keys de empresas no activas.
7. **Ventana de access token.** Los access tokens ya emitidos siguen válidos
   hasta su TTL corto (900 s por defecto); el refresh queda bloqueado de
   inmediato. Para revocación instantánea haría falta consultar el estado en el
   gateway o un `token_version` por tenant.

## Consecuencias

- **Positivas:** suspender un tenant corta el refresh y las nuevas sesiones;
  el cierre queda auditado; no se depende de borrar usuarios a mano.
- **Contrato:** nueva ruta y estados; errores específicos de empresa.
- **Datos:** sin migración; se reutilizan `empresas.estado`,
  `sesiones.revocada_en` y `auditoria`.
- **Pendiente:** la exportación y el borrado de identidad se implementaron en
  [ADR-0035](./0035-exportacion-borrado-retencion-tenant.md); la orquestación
  cross-service (orders, catalog, inventory, finance, etc.) sigue pendiente.

## Criterios de aceptación

- Un admin puede suspender, cerrar y reactivar una empresa.
- Un empresario no puede suspender su propia empresa.
- Tras suspender, el login responde `EMPRESA_SUSPENDIDA` y el refresh falla.
- Tras reactivar, el login vuelve a funcionar.
- `inactiva → suspendida` responde `TRANSICION_ESTADO_INVALIDA`.
- El cambio queda en `identity.auditoria`.
