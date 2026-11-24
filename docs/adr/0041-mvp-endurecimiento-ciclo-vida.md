# ADR-0041 — Endurecimiento y ciclo de vida para el MVP vendible

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0035](./0035-exportacion-borrado-retencion-tenant.md),
  [ADR-0036](./0036-endurecimiento-autenticacion.md),
  [ADR-0039](./0039-rma-reembolso-nota-credito.md),
  [ADR-0040](./0040-pdf-comprobantes.md)

## Contexto

El núcleo funcional estaba completo, pero quedaban límites que impedían venderlo
como SaaS operable: secretos MFA en claro, sin reset administrativo de MFA, la
verificación de correo no bloqueaba el primer token, no se registraba la IP de
sesión, la purga de tenant solo cubría identity, los ficheros de storage no se
borraban, la auditoría crecía sin política de retención y la nota de crédito no
pasaba por el proveedor electrónico.

## Decisión

1. **MFA cifrado en reposo.** `mfa_secreto` se guarda con AES-256-GCM
   (`v1:<nonce>:<ciphertext>`). La clave viene de `MFA_ENCRYPTION_KEY`
   (base64 de 32 bytes) y es obligatoria en producción. Los valores legacy en
   claro se aceptan y se migran al confirmar el MFA.
2. **Reset administrativo de MFA.** `POST /api/v2/usuarios/{id}/mfa/reset`
   (super_admin/admin) limpia secreto, códigos de recuperación y sesiones.
3. **Registro sin token si el correo es obligatorio.** Con
   `AUTH_EXIGIR_CORREO_VERIFICADO=true`, `POST /api/v2/auth/registro` devuelve
   `{usuario, verificacion_requerida: true}` sin access/refresh token. El usuario
   debe verificar y luego iniciar sesión.
4. **IP de sesión con proxies confiables.** El gateway solo usa
   `X-Forwarded-For` si `TRUST_FORWARDED_FOR=true`; en caso contrario usa
   `X-Real-IP`. Reenvía la IP resuelta como `X-Client-IP` e identity la persiste
   en `identity.sesiones.ip` y la expone en el listado de sesiones.
5. **Purga de tenant coordinada.** Tras anonimizar identity, se hace fan-out a
   `TENANT_PURGE_SERVICES` (`nombre=url,...`) con
   `POST /internal/{servicio}/tenant/purgar` y `x-internal-key`. El resultado se reporta
   por servicio; un fallo no revierte la purga de identidad.
   - `storage`: borra objetos, ficheros físicos y uso de cuota.
   - `orders`: elimina datos efímeros y anonimiza PII; conserva órdenes/pagos.
   - `finance`: anonimiza `datos_cliente`, borra PDFs y configuración fiscal;
     conserva comprobantes/asientos por retención contable.
   La llamada es reintentable: si un servicio falla, se puede repetir
   `POST /empresas/{id}/eliminacion/purgar` sobre una solicitud ya `purgada`.
6. **Retención de auditoría.** `AUDITORIA_RETENCION_DIAS` (default 365, `0` =
   conservar) y `POST /api/v2/admin/auditoria/retencion` (super_admin).
7. **Nota de crédito electrónica.** `emitir_nota_credito` usa el mismo proveedor
   electrónico configurado para facturas y guarda
   `electronica_estado`/`electronica_referencia`.

## Consecuencias

- **Positivas:** los límites de seguridad y ciclo de vida quedan cubiertos para
  operar el MVP; la purga borra ficheros y anonimiza datos comerciales.
- **Contrato:** nuevos endpoints de reset MFA, retención y purga interna; el
  registro puede devolver una respuesta sin tokens.
- **Operación:** `MFA_ENCRYPTION_KEY` y `TENANT_PURGE_SERVICES` son variables de
  despliegue; sin `TENANT_PURGE_SERVICES` solo se purga storage.
- **Límite:** orders/finance conservan registros contables anonimizados por
  obligación legal.
- **Exportación cross-service:** implementada en ADR-0042 (job asíncrono y archivo firmado).
- **PDF:** en ADR-0042 se mueve a object storage con fallback a PostgreSQL.

## Criterios de aceptación

- El secreto MFA no aparece en claro en la base de datos.
- Un admin puede resetear el MFA y las sesiones del usuario.
- Con verificación obligatoria, el registro no entrega tokens.
- La sesión registra la IP cuando el proxy confiable la envía.
- La purga de storage borra los ficheros y deja la cuota en cero.
- Orders/finance anonimizan PII y no pierden comprobantes.
- La auditoría antigua se elimina según la retención configurada.
- La nota de crédito registra estado electrónico cuando el tenant lo activa.
