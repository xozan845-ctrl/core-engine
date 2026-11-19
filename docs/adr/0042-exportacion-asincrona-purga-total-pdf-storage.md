# ADR-0042 — Exportación asíncrona firmada, purga total y PDF en object storage

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0035](./0035-exportacion-borrado-retencion-tenant.md),
  [ADR-0040](./0040-pdf-comprobantes.md),
  [ADR-0041](./0041-mvp-endurecimiento-ciclo-vida.md)

## Contexto

ADR-0041 cerró los límites de seguridad y una purga parcial. Quedaban cuatro
límites que impedían afirmar que el MVP no retiene datos fuera de control:

1. la exportación era síncrona y solo cubría identity;
2. la purga solo llegaba a storage, orders y finance;
3. el PDF se guardaba en PostgreSQL;
4. la retención de auditoría solo borraba filas.

## Decisión

1. **Exportación asíncrona cross-service.**
   - `POST /api/v2/empresas/{id}/exportacion` crea un job (`identity.exportacion_jobs`)
     y responde `201`.
   - Un worker `tokio` recolecta identity y hace fan-out a
     `POST /internal/{servicio}/tenant/exportar`.
   - El archivo es `core-engine.tenant-export.v2` con
     `identidad` y `servicios`.
   - Se firma con HMAC-SHA256 (`EXPORT_SIGNING_KEY`, fallback `JWT_SECRET`) y se
     guarda `sha256`/`firma`.
   - `GET /api/v2/empresas/{id}/exportacion/{job}` consulta el estado.
   - `GET /api/v2/exportaciones/{job}/descarga` descarga el archivo con
     `x-core-sha256`, `x-core-signature` y `x-core-format`.
2. **Contrato único de ciclo de vida en todos los servicios.**
   - `POST /internal/{servicio}/tenant/exportar`.
   - `POST /internal/{servicio}/tenant/purgar`.
   - Implementado en orders, finance, inventory, item, pricing, stores,
     commissions, logistics, scheduling, analytics, field, intelligence,
     storage y notifications.
   - `TENANT_PURGE_SERVICES` define `nombre=url`; el orden se respeta para
     satisfacer FKs (commissions antes de orders, inventory/pricing antes de
     item).
3. **PDF en object storage.**
   - Nuevo endpoint interno `POST /internal/storage/objetos` para subida directa
     de bytes y `GET /internal/storage/objetos/{id}/contenido` para lectura.
   - Finance sube el PDF a storage cuando `PDF_ALMACENAMIENTO=storage` y guarda
     `objeto_id` en `finance.comprobante_pdf`.
   - Si storage falla, cae a `bytea` en PostgreSQL (degradación controlada).
4. **Retención de auditoría con modo legal.**
   - `AUDITORIA_RETENCION_MODO=anonimizar` (default) conserva acción, recurso y
     fechas, y redacta actor, request_id y datos.
   - `AUDITORIA_RETENCION_MODO=eliminar` borra las filas antiguas.
5. **Auditoría del tenant purgado.** Al purgar se redactan `actor_usuario_id`,
   `request_id` y `datos` de los eventos previos; se conserva acción, recurso y
   fecha, y se registra `empresa.eliminacion_purgada`.
6. **Servicios sin datos de tenant.** `notifications` responde `no_aplica`.
   `intelligence.metricas_producto` es un agregado global por SKU sin PII y se
   conserva.

## Consecuencias

- **Positivas:** el tenant puede exportar todo su dato en un archivo firmado y
  la purga borra/anonimiza en todos los servicios; el PDF ya no pesa en la base.
- **Datos:** migraciones `99_exportacion_jobs.sql` y
  `99_comprobante_pdf_storage.sql`.
- **Operación:** `TENANT_PURGE_SERVICES` debe listar los servicios desplegados;
  `PDF_ALMACENAMIENTO=storage` exige `STORAGE_SERVICE_URL`.
- **Límite acotado:** en el archivo, los objetos de storage de hasta 5 MiB se
  incluyen en base64; los mayores se referencian por id para no generar
  archivos sin límite. Los PDFs fiscales en base también se incluyen.
- **Límite legal:** `cobertura_zona` (agregado geográfico) y los registros
  contables anonimizados se conservan por retención legal.
- **Límite:** la firma HMAC permite verificar integridad, no sustituye una
  firma electrónica legal.

## Criterios de aceptación

- Un admin crea un job, espera `completado` y descarga un archivo con firma.
- El archivo contiene identidad y datos de los servicios configurados.
- La purga borra ficheros de storage y datos de todos los servicios listados.
- Un fallo de un servicio se reporta sin impedir el resto.
- El PDF se sirve desde storage y cae a base si el almacén no está disponible.
- La auditoría antigua se anonimiza o elimina según configuración.
