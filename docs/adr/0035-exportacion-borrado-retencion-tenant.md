# ADR-0035 — Exportación, borrado y retención del tenant

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado; orquestación cross-service en ADR-0041)
- **Relacionado:** [ADR-0034](./0034-ciclo-vida-tenant.md)

## Contexto

El cierre de una empresa no tenía un proceso de exportación, eliminación ni
retención. Borrar `identity.empresas` en seco rompería referencias de otros
servicios (`orders`, `catalog`, `inventory`, `finance`, etc.) y no cubriría los
datos que esos servicios poseen.

## Decisión

1. **Exportación de identidad.** `GET /api/v2/empresas/{id}/exportacion`
   devuelve un JSON versionado (`core-engine.tenant-export.v1`) con empresa,
   asignaciones, usuarios sin `contrasena_hash`, negocios, partes, contactos,
   direcciones, relaciones, capacidades, extensiones, slugs y auditoría.
   El campo `alcance.pendiente` lista los servicios que aún deben aportar datos.
2. **Solicitud de eliminación.** `POST /api/v2/empresas/{id}/eliminacion` con
   `{motivo?, retencion_dias?}` (default 30, máximo 3650). Cierra el tenant
   (`inactiva`), revoca sesiones y crea
   `identity.solicitudes_eliminacion_tenant`.
3. **Cancelación.** `DELETE .../eliminacion` cancela una solicitud pendiente
   antes de que venza la retención. El tenant sigue `inactiva` hasta que un
   administrador lo reactive.
4. **Purga de identidad.** `POST .../eliminacion/purgar` (solo `super_admin`)
   solo funciona si `retencion_hasta <= CURRENT_DATE`. Anonimiza:
   - empresa (nombre, slug, datos fiscales/comerciales, tema, media);
   - usuarios del tenant (correo, nombre, hash, avatar y participación);
   - partes, contactos, direcciones, relaciones y registros;
   - valores de extensiones y nombres de negocios;
   - elimina sesiones, API keys, asignaciones, invitaciones y slugs históricos.
   Conserva las filas ancla (`empresas`, `usuarios`, `partes`) para no romper
   referencias cross-service.
5. **Auditoría.** Se registran `empresa.export`, `empresa.eliminacion_solicitada`,
   `empresa.eliminacion_cancelada` y `empresa.eliminacion_purgada`.
6. **Retención.** La purga no es automática: exige una acción administrativa
   después del vencimiento. Esto evita borrados accidentales y permite revisar
   la solicitud.

## Consecuencias

- **Positivas:** hay un proceso auditable de exportación y borrado de identidad
  sin romper FKs; la retención es explícita.
- **Datos:** migración `99_tenant_eliminacion.sql`; tabla
  `identity.solicitudes_eliminacion_tenant`; RLS solo para el rol de servicio.
- **Orquestación cross-service:** implementada en ADR-0041 mediante
  `TENANT_PURGE_SERVICES`. Storage borra ficheros; orders y finance anonimizan
  PII conservando registros contables. Otros servicios pueden sumarse al mismo
  contrato `POST /internal/{servicio}/tenant/purgar`.
- **Retención de auditoría:** `AUDITORIA_RETENCION_DIAS` y
  `POST /api/v2/admin/auditoria/retencion` (ADR-0041).
- **Límite:** la exportación es síncrona y de identidad; no genera un archivo
  firmado ni un ZIP. Para volúmenes grandes conviene un job asíncrono.

## Criterios de aceptación

- Un admin exporta la identidad y no ve hashes de contraseña.
- Solicitar eliminación cierra el tenant y bloquea login.
- No se puede duplicar una solicitud pendiente.
- Se puede cancelar antes de la retención y reactivar la empresa.
- No se puede purgar antes de la retención.
- La purga anonimiza empresa, usuarios y partes, y deja la solicitud en
  `purgada`.
