# Backlog de plataforma y escala (2026-11-20)

Este documento cierra la auditoría de plataforma: **P0, P1 y P3 están
implementados**; lo que sigue es el backlog acordado, con estado actual,
disparador para hacerlo y criterio de aceptación. Vive aquí —no en READMEs—
para que las decisiones no se diluyan (R-DO-7).

## Implementado en este ciclo

| Área | Entrega | ADR / runbook |
|---|---|---|
| Backup/restore | Dump diario, retención, offsite manual y restore drill | [ADR-0044](./adr/0044-backup-restore-verificado.md), [runbook](./runbook-backup-restore.md) |
| Webhooks salientes | Suscripciones por tenant, HMAC, reintentos, API admin | [ADR-0045](./adr/0045-webhooks-salientes.md) |
| Cobro de plataforma | Plan `empresario_plus` USD 35/mes para >1 negocio, cobro manual | [ADR-0046](./adr/0046-suscripcion-plataforma-expansion.md) |

## Backlog priorizado

### P1 — Endurecimiento operativo y de integración

| # | Tema | Estado actual | Disparador | Diseño mínimo | Aceptación |
|---|---|---|---|---|---|
| 1 | **PITR real** | Dump lógico diario; RPO 24 h | Un tenant con volumen alto o requisito de RPO < 24 h | `wal_level=replica`, `archive_mode=on`, archivado WAL fuera del host, `pg_basebackup`/pgBackRest y restore drill con timestamp | Restaurar a un minuto concreto en staging y documentar RPO/RTO |
| 2 | **Scopes de API keys + auditoría de eventos** | La key solo tiene `rol`; rotación manual; `creado_por`/`ultimo_uso_en`, sin eventos de auditoría | Integrador tercero o requisito de mínimo privilegio | Columna `scopes text[]` (o tabla), incluir en JWT, enforce en gateway/servicios, registrar creación/rotación/revocación/uso en `identity.auditoria` | Una key con scopes limitados no puede llamar rutas fuera de su alcance; la auditoría muestra cada evento |
| 3 | **SSRF hardening de webhooks** | Valida esquema/host; no bloquea IP privada ni DNS rebinding | Primer webhook a un destino no confiable o auditoría de seguridad | Resolver DNS y rechazar rangos privados/loopback/link-local, permitir allowlist por tenant, egress proxy | Alta de `http://127.0.0.1` rechazada; allowlist documentada |
| 4 | **Circuit breaker, reintentos y timeout por ruta en gateway** | 502 de transporte sin estado abierto ni presupuesto por ruta | Latencia de un upstream afectando al resto | Estados cerrado/abierto/semiabierto, retry idempotente y timeout por destino | Un upstream caído no arrastra al gateway y se recupera solo |
| 5 | **Rotación de claves JWT (`kid`)** | HS256 único, sin `kid` ni rotación | Necesidad de rotar el secreto sin invalidar sesiones | `kid` en header, mapa de claves activas/antiguas y ventana de convivencia | Rotar el secreto sin cortar sesiones válidas |

### P2 — Escala horizontal

| # | Tema | Estado actual | Disparador | Diseño mínimo | Aceptación |
|---|---|---|---|---|---|
| 6 | **Rate limit y caché distribuidos** | `RateLimit` en memoria por réplica; solo caché de readiness | Más de una réplica del gateway o picos entre instancias | Redis (o Postgres con `pg_advisory`/tabla TTL) para ventanas y caché de lectura | Dos réplicas comparten el mismo límite y no duplican conteos |
| 7 | **Réplicas de lectura** | Una `DATABASE_URL` para lectura/escritura | Reportes/analytics compitiendo con checkout | `DATABASE_REPLICA_URL` y pool de lectura separado por servicio; consistencia explícita | Reportes leen de réplica sin afectar la latencia de escritura |
| 8 | **Búsqueda dedicada** | Trigram GIN en PostgreSQL (`99_busqueda_trigram.sql`) | Catálogo > ~1M ítems o búsqueda semántica/facetada | Índice externo (OpenSearch/Meilisearch) con outbox de indexación y fallback a SQL | Búsqueda con facetas y relevancia sin degradar checkout |
| 9 | **OpenTelemetry real** | `OTEL_EXPORTER_OTLP_ENDPOINT` en `.env*`, sin crate ni exportador; logs JSON + Prometheus/Grafana propios | Necesidad de trazas entre servicios o correlación con APM | `tracing` + `opentelemetry-otlp` en un crate compartido, propagación W3C y muestreo configurable | Una orden se sigue de gateway a finance en el APM |

### P3 — Producto/plataforma

| # | Tema | Estado actual | Disparador | Diseño mínimo | Aceptación |
|---|---|---|---|---|---|
| 10 | **Pasarela de cobro de la suscripción** | Cobro manual con confirmación admin | Volumen de altas o cobro recurrente automático | Adaptador Stripe/BAC/POS sobre `suscripciones_plataforma`, webhook firmado y conciliación | Un pago activa/renueva sin intervención manual |
| 11 | **Factura/impuestos de la suscripción** | No se emite comprobante de plataforma | Requisito fiscal del SaaS | Serie/impuesto de plataforma separado de la facturación del tenant | El tenant descarga su comprobante de suscripción |
| 12 | **Plugin/SDK de capacidades** | Capacidad nueva = migración + código + `features.rs` + guard (ADR-0001); `packs` compone y `extensiones` da esquemas JSON | Un tercero debe añadir una capacidad sin tocar el core | Contrato de plugin (WASM/sidecar), registro firmado, versión y sandbox | Un plugin se registra/activa por tenant sin recompilar el core |
| 13 | **Rotación de refresh tokens** | Misma `jti` al renovar (`docs/decisiones.md`) | Requisito de seguridad de sesiones | Rotación con detección de reuso y cierre selectivo | Un refresh reutilizado revoca la familia |

## Qué no se hará ahora (y por qué)

- **No** se introduce Redis solo para el rate limit: el despliegue actual es de
  una réplica y el propio README del gateway lo documenta como “por réplica”.
- **No** se cambia PostgreSQL por un motor de búsqueda: el trigram cubre el MVP
  y el costo operativo de un índice externo no se justifica todavía.
- **No** se convierte la plataforma en un motor de plugins: ADR-0001 decide
  explícitamente no construir un ERP universal ni un workflow genérico.
- **No** se activa PITR sin ventana de mantenimiento y restore drill: un
  `archive_command` mal configurado es peor que no tener PITR.

## Cómo se prioriza

1. Un ítem entra al sprint solo si tiene **disparador de negocio** (contrato,
   incidente o límite medido), no por completitud.
2. Toda capacidad nueva sigue el proceso de ADR-0001: migración aditiva + ADR +
   OpenAPI + `features.rs` + guard + tests.
3. Los ítems de seguridad (PITR, scopes, SSRF, `kid`) se revisan en cada
   promoción a producción.
