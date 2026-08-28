# Decisiones de Arquitectura (ADR) — BodegaHub Core Engine

Este archivo registra decisiones técnicas deliberadas que se apartan del texto literal del documento de referencia (BodegaHub_Informe.pdf) o que requieren justificación para auditoría. Cada decisión se enmarca como **ADR-XX** con estado, contexto y consecuencias.

---

## ADR-01: Montos en centavos (`*_cents`) — desviación de nombres de contrato

- **Fecha**: 2026-08-09
- **Estado**: Aceptado
- **Contexto**: El documento (Tabla 22, 5.7) especifica campos como `total`, `monto` en los payloads de eventos y respuestas. La implementación usa `total_cents`, `monto_cents`, `precio_base_cents`, etc.
- **Decisión**: Mantener la convención `*_cents` (enteros) en todo el backend.
- **Justificación**:
  - Control OWASP A02: evitar aritmética de punto flotante en montos monetarios.
  - `Money` class encapsula la serialización `"1150.00"` (dos decimales) en `string()`.
  - Cambiar a nombres sin `_cents` rompería la seguridad de tipos y requeriría conversiones en cada límite de servicio.
- **Mitigación**: La API expone montos formateados en respuestas HTTP; los contratos de eventos internos usan centavos. En la documentación OpenAPI/Swagger los campos aparecen con `format: "currency"` y ejemplos en C$ con dos decimales.

---

## ADR-02: Límite de margen RN-01 (0–90 %)

- **Fecha**: 2026-08-09
- **Estado**: Corregido (era 0–100 %)
- **Contexto**: `Money.aplicarMargen` aceptaba 100 % en `shared/src/money.ts:58`, pero RN-01 y `MARGEN_MAXIMO = 90` en `constants.ts` exigen 90 %.
- **Acción**: Cambiado el límite a 90 y mensaje de error acorde. Test unitario actualizado.
- **Impacto**: Ninguno en producción (los servicios validaban contra `MARGEN_MAXIMO`); el helper compartido ahora es consistente.

---

## ADR-03: Rol "logística" — no existe en el MVP

- **Fecha**: 2026-08-09
- **Estado**: Documentado
- **Contexto**: El documento (Tabla 21, 4.3) menciona rol "logística" para avanzar estado de orden (`PATCH /api/v1/orders/:id/estado`). En la implementación los roles son `admin`, `vendedor`, `comprador` (Tabla 15 RLS).
- **Decisión**: Las rutas de logística exigen `admin`. El servicio `logistics-service` existe y consume eventos (`payment.procesado`, `order.status.updated`), pero no hay un rol JWT separado.
- **Justificación**: Simplifica RBAC y RLS en el MVP; el admin cubre la operación de bodega. Si se requiere separación, se añade el rol en `ROLES` y migraciones RLS sin romper APIs.

---

## ADR-04: Frontend Angular + Capacitor — pendiente

- **Fecha**: 2026-08-09
- **Estado**: Pendiente (entregable separado)
- **Contexto**: El documento (AD-07, Cap. 3.3, Tabla 30) exige SPA Angular servida en web y envuelta con Capacitor para Android/iOS.
- **Decisión**: No incluido en este repositorio (backend only). El frontend se desarrollará en repositorio aparte o carpeta `frontend/` futura.
- **Impacto**: RN-05 (carrito 30 min) se implementa en backend (orders-service) para permitir API-first; el frontend consumirá `/api/v1/carrito`.

---

## ADR-05: Rate limiting real en Gateway (429 estructurado)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: El documento (5.4, 5.7) exige limitación de tasa en el gateway y error 429 con formato `{codigo, mensaje, detalles}`. `ThrottlerGuard` de Nest estaba declarado pero inerte (gateway sin controllers).
- **Decisión**: `RateLimitMiddleware` en memoria (ventana fija por IP): global 300 req/min, login 10 req/min. Respuesta 429 con `codigo: 'DEMASIADAS_PETICIONES'`, cabeceras `x-ratelimit-*` y `Retry-After`. `ThrottlerModule` removido.
- **Consecuencia**: El 429 ahora es real y estructurado.

---

## ADR-06: Normalización total de errores HTTP (doc 5.7)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `DomainErrorFilter` dejaba pasar `HttpException` de Nest tal cual (sin `{codigo, mensaje, detalles}`).
- **Decisión**: Todo error (DomainError, HttpException, 500) sale normalizado. Mapeo status→codigo: 400 `SOLICITUD_INVALIDA`, 401 `NO_AUTORIZADO`, 403 `ACCESO_DENEGADO`, 404 `NO_ENCONTRADO`, 409 `CONFLICTO`, 429 `DEMASIADAS_PETICIONES`, else `HTTP_<status>`.

---

## ADR-07: Trazabilidad y métricas en TODOS los microservicios

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `TrazabilidadInterceptor` solo estaba en gateway (como `APP_INTERCEPTOR`); los microservicios no registraban `http_peticion_*` ni propagaban `request_id` a logs.
- **Decisión**:
  - `TrazabilidadInterceptor` ahora envuelve el handler en `ejecutarConContexto` → `request_id` disponible en `Logger` vía `AsyncLocalStorage`.
  - Cada `main.ts` registra `app.useGlobalInterceptors(new TrazabilidadInterceptor(metrics, NOMBRE_SERVICIOS.X))`.
  - Gateway: `PasarelaMiddleware` genera `x-request-id` y registra métricas en `finish`.

---

## ADR-08: Reintento DLQ vivo con backoff exponencial (AD-04 / TC-06)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `RabbitService.reintentarDesdeDlq` existía pero nunca se invocaba (código muerto). El documento exige backoff y reinyección automática.
- **Decisión**:
  - `RabbitService` rastrea colas declaradas y expone `activarReintento(intervaloMs=10s)` con poller `setInterval`.
  - Al dead-letter se guarda `x-routing-key-original` para reinyectar al exchange de eventos con la routing key correcta.
  - Todos los consumidores (catalog, orders, logistics, commissions, finance, stores) llaman `rabbit.activarReintento()` en `onModuleInit`.

---

## ADR-09: RN-02 — Oferta "agotada" al llegar stock 0 (consumidor real)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: `OfertasService.sincronizarStockDeOferta` existía pero no se invocaba (código muerto). No había consumidor de `stock.updated`.
- **Decisión**:
  - `catalog-service` al reservar/reintegrar stock publica `stock.updated` con `{items: [{sku, stock_restante}]}`.
  - `stores-service` nuevo `StockConsumer` consume `stock.updated` y llama `sincronizarStockDeOferta(sku, stock_restante)` con idempotencia (`stores.eventos_procesados`).
  - Cola `stores.stock` añadida en `rabbit.constants.ts`.

---

## ADR-10: RN-05 — Carrito con expiración 30 min (backend)

- **Fecha**: 2026-08-09
- **Estado**: Implementado
- **Contexto**: El documento trata el carrito como concepto de frontend (SPA) sin endpoints ni tabla. La auditoría exigía entidad + endpoints + expiración.
- **Decisión**: `orders.carritos` (PK `comprador_id`, `items_json`, `total_cents`, `actualizado_en`) con purga perezosa `actualizado_en > NOW() - 30 min`. Endpoints `/api/v1/carrito` (GET, POST items, PATCH item, DELETE item, DELETE all) bajo rol `comprador`. Checkout opcional con `usar_carrito: true` vacía el carrito en la misma transacción que crea la orden.
- **RLS**: Comprador solo su carrito; admin acceso total.

---

## ADR-11: Migraciones SQL y RLS fuera de Docker (doc AD-08)

- **Fecha**: 2026-08-09
- **Estado**: Confirmado
- **Contexto**: `infra/db/init/01_esquemas.sql` se monta en `docker-entrypoint-initdb.d` (Docker local). `infra/db/supabase/99_rls.sql` se aplica manualmente en staging/prod (Supabase provee `auth.uid()/auth.rol()`).
- **Acción**: Añadidas tablas `orders.carritos` e `stores.eventos_procesados` en `01_esquemas.sql` + políticas RLS en `99_rls.sql`.

---

## Resumen de cumplimiento post-corrección

| Ítem del documento          | Estado  | Nota |
|----------------------------|---------|------|
| RN-01 (margen 0-90)        | ✅      | helper y servicios alineados |
| RN-02 (oferta agotada)     | ✅      | consumidor `stock.updated` vivo |
| RN-03 (descuento atómico)  | ✅      | transacción catalog + evento |
| RN-04 (comisión 12 %)      | ✅      | `Money.comision(0.12)` |
| RN-05 (carrito 30 min)     | ✅      | tabla + endpoints + expiración |
| RN-06 (devolución stock)   | ✅      | `stock.reintegrado` + `stock.updated` |
| RN-07 (liquidación 1/15)   | ✅      | `LIQUIDACION_DIAS = [1,15]` |
| RN-08 (histórico precios)  | ✅      | `historico_precios` en catalog y stores |
| Tabla 13 (ciclo orden)     | ✅      | `puedeTransicionar` + handler |
| Tabla 21 (10 endpoints)    | ✅      | + carrito endpoints |
| Tabla 22 (eventos)         | ✅      | nombres exactos; `stock.updated` extendido |
| AD-02 (RabbitMQ + DLQ)     | ✅      | exchanges + colas + DLQ |
| AD-03 (Outbox)             | ✅      | tabla por servicio + poller |
| AD-04 (backoff DLQ)        | ✅      | poller 10s + reinyección |
| 5.3 (métricas + request_id)| ✅      | todos los servicios |
| 5.4 (gateway rate limit)   | ✅      | 429 real + cabeceras |
| 5.7 (errores estructurados)| ✅      | normalizador universal |
| 4.3/RLS (seguridad)        | ✅      | esquemas + políticas nuevas |

---

*Generado tras auditoría 2026-08-09. Todas las correcciones verificadas con `npm run build`, `npm run test`, `npm run lint`.*