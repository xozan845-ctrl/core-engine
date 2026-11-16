# ADR-0020: `analytics-service` como único dueño de la analítica

- **Fecha**: 2026-11-16
- **Estado**: Aceptado — implementación inicial con negocio, empresa y plataforma
- **Ámbito**: analítica de gestión, cohortes, reportes programados
- **Referencias**: [ADR-0009](./0009-market-intelligence-service-en-rust.md),
  [ADR-0019](./0019-negocios-por-empresa.md)

## Contexto

La analítica nació dentro de `orders-service` porque las líneas de la orden son
la fuente de hechos. Después se movió a `commissions-service` para aprovechar su
consumidor de `order.completado`. Ambos enfoques dejan el dominio repartido:

- `orders-service` conoce datos de venta, pero no debería calcular analítica.
- `commissions-service` conoce comisiones, pero no debería ser el dueño de la
  analítica de negocio.
- `market-intelligence-service` ya cubre otra analítica: mapas de calor,
  cobertura, demanda y forecast.

El requisito es que **un solo microservicio sepa de analítica de gestión**:
por negocio, por empresa y por plataforma.

## Decisión

Crear `analytics-service`, con esquema propio `analytics`, como excepción OLAP
autorizada (junto con `market-intelligence-service`).

### Responsabilidades

- Ingesta event-driven:
  - `order.completado` → consulta la orden por la superficie interna de
    `orders-service` y proyecta `analytics.hechos_venta`.
  - `comision.acreditada` → proyecta `analytics.comisiones`.
  - Idempotencia en `analytics.eventos_procesados`.
- Consultas:
  - `GET /api/v2/analytics/negocios`
  - `GET /api/v2/analytics/negocios/cohortes`
  - `GET /api/v2/analytics/empresa`
  - `GET /api/v2/analytics/plataforma` (solo `super_admin`/`admin`)
- Reportes programados y su historial.
- Capacidad `analitica_negocios` en identity, activa por defecto.

### Límites

- `orders-service` vuelve a ser solo fuente de hechos.
- `commissions-service` vuelve a comisiones y liquidaciones.
- `market-intelligence-service` conserva mercado, geoespacial y predicción.
- `analytics-service` no renderiza dashboards; entrega JSON/CSV al frontend.

## Consecuencias

### Positivas

- Un único dueño del dominio analítico de gestión.
- Ingesta desacoplada de `orders-service`; escalable y repetible por eventos.
- El frontend tiene un contrato único `/api/v2/analytics/**`.
- `market-intelligence-service` no se sobrecarga con contabilidad operativa.

### Costes

- Nuevo servicio, Dockerfile, compose, CI y OpenAPI.
- Consistencia eventual: la analítica refleja órdenes completadas, no el estado
  transitorio del checkout.
- Backfill inicial necesario para el histórico.
