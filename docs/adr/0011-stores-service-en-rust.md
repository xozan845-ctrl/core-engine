# ADR-0011: Reescritura de `stores-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación en curso
- **Ámbito**: `stores-service` (HTTP público e interno, PostgreSQL/RLS,
  RabbitMQ, herencia de tema y validación contra `storage`/`catalog`)
- **Referencias**: [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0004](./0004-scheduling-service-en-rust.md),
  [ADR-0006](./0006-notifications-service-en-rust.md),
  [ADR-0008](./0008-field-service-en-rust.md),
  [ADR-0010](./0010-logistics-service-en-rust.md)

## Contexto

`stores-service` es el servicio NestJS de tiendas y ofertas (~3.700 líneas de
TypeScript no-spec) que:

- Expone la tienda del vendedor, el listado de tiendas de la empresa, el slug
  público con redirección 301, el CSS del tema, el logo, los fondos de
  escritorio y móvil, la personalización (plantilla, tema, hero y SEO) y el
  contenido del storefront. Además publica ofertas de productos, servicios y
  alquileres y sirve la tienda pública con sus ofertas.
- Mantiene la superficie interna `internal/tiendas/**` (lote por ids, tienda
  por vendedor/empresa, asignación y compensación de vendedor) y
  `internal/ofertas/**` (lote por ids y conteo de ofertas activas), protegida
  por `x-internal-key`.
- Resuelve la herencia visual `plataforma → empresa → tienda` consultando los
  endpoints internos de tema de `identity-service`, con caché TTL y degradación
  a defaults si identity no responde.
- Valida los objetos de storage (logo, fondo, poster, imagen SEO) contra la
  metadata interna de `storage-service`; el binario nunca pasa por `stores`.
- Verifica el item del catálogo unificado en `catalog-service` y aplica RN-01
  (precio de venta = base × (1 + margen), margen 0–90) y RN-02 (no publicar sin
  stock).
- Consume la cola `stores.stock` (`stock.updated`) con idempotencia por
  `event_id` para sincronizar el stock visible y el estado `agotada`.
- Emite `envelope {data, meta}` (R-C-1), errores `{codigo, mensaje, detalles?}`,
  métricas Prometheus y `/health`.

El servicio está **sin migración en curso**: no existe rama, worktree, ADR ni
sesión de trabajo sobre `stores-service` distinta de esta. Las migraciones
activas son `field-service` (ADR-0008), `logistics-service` (ADR-0010) y
`market-intelligence-service` (ADR-0009), todas ya integradas. Se elige
`stores-service` por su aislamiento relativo (sin event sourcing ni pagos), su
contrato dorado de tienda/oferta y porque replica el patrón PG + RLS +
RabbitMQ + capacidades ya validado en ADR-0006/ADR-0010.

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismas rutas, métodos, códigos (201 en POST, 200 en GET/PATCH, 301 en slug anterior) y mismos cuerpos; envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores sin `meta` |
| Superficie interna | `GET /internal/ofertas`, `/internal/ofertas/producto/:id/activas`, `/internal/tiendas`, `/internal/tiendas/empresa/:id`, `/internal/tiendas/vendedor/:id`, `/internal/tiendas/:id`, `POST /internal/tiendas/:id/asignar-vendedor` y `/desasignar-vendedor`; sin envelope; `403 ACCESO_DENEGADO` con clave inválida |
| Datos | mismas tablas y columnas (`stores.tiendas`, `stores.ofertas`, `stores.tienda_slugs`, `stores.historico_precios`, `stores.eventos_procesados`), mismos tipos JSON de `node-postgres` (uuid/texto string, `int4`/`smallint` número, `jsonb` objeto/array, `timestamptz` ISO 8601 con milisegundos y `Z`) |
| Multi-tenancy | `tenant_id` derivado de `x-tenant`; claims `sub`/`rol`/`tenant_id` en `request.jwt.claims` dentro de la transacción para RLS (R-DB-2/R-SB-3) |
| Autorización | mismos roles por ruta; `super_admin`/`admin` omiten capacidades; `empresario` exige tenant para crear/listar; `vendedor` exige `participacion_porcentaje`; rol configurable solo pasa con `@Permisos` declarados; capacidad `catalogo_items` en ofertas; identity caído ⇒ `503 SERVICIO_NO_DISPONIBLE`; capacidad faltante ⇒ `403 CAPACIDAD_NO_ACTIVA` con `{empresa_id, capacidades}` |
| Tienda | nombre 2–100; slug explícito 2–80 normalizado, reservados y UUID rechazados; colisión de slug `409`; sin slug explícito se reintenta 3 veces y luego se elige uno libre; slug anterior redirige 301 a `/api/v2/tiendas/<slug actual>` |
| Acceso a tienda | logo/fondo/personalización/contenido: `super_admin`, `admin`, `empresario` del mismo tenant o `vendedor` propietario/asignado; mensajes y códigos del TS |
| Tema | plantillas `clasica/minimal/boutique/mercado`, fuentes `system/inter/poppins/montserrat/playfair`, modos `claro/oscuro/auto`; colores hex normalizados a mayúsculas; radio 0–32; herencia y `origen_personalizacion` idénticos; `GET /api/v2/tiendas/:id/tema.css` con `text/css; charset=utf-8` y `Cache-Control: public, max-age=60` |
| Fondo | `background_*` y `background_movil_*` con defaults (`overlay=35`, `position=center`, `activo=false`); `null` limpia el bloque; poster exige fondo; media type se deriva del objeto validado (`imagen`/`video`); URLs públicas `/api/v2/storage/public/objetos/<id>` |
| Personalización | hero/SEO con límites 80/160/40/300/70/300; CTA y enlaces solo ruta relativa segura o HTTPS; `null` en plantilla/tema limpia el override |
| Contenido | secciones máx. 12, tipos y órdenes únicos; sobre nosotros 2000; políticas 1000; redes máx. 6 con URL HTTPS y tipo único; anuncio 120/300 |
| Ofertas | RN-01 margen entero 0–90 y centavos; RN-02 stock < 1 rechazado si el item controla inventario; item debe ser de la misma empresa, activo y existir en catalog; sin duplicado `(tienda, item_type, item_id)`; moneda ISO 3 letras (default `NIO`); histórico de precios en cada alta/cambio |
| Consumidor | cola `stores.stock`, routing `stock.updated`; idempotencia por `event_id`; sincroniza `LOWER(sku)` de ofertas `producto` de la empresa y deja `activa`/`agotada` con stock `max(0, n)` |
| RabbitMQ | exchanges `core-engine.events`/`core-engine.dlq`; DLQ por cola; `x-intentos` máx. 3 y republicación cada 10 s; prefetch 1 |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus con `core_engine_http_*` y etiquetas `{servicio, metodo, ruta, estado}`; logs JSON con `request_id`, `servicio` y nivel; `/docs` y `/docs-json` con el OpenAPI versionado |
| Runtime | puerto 3003 (`PORT`), variables `DATABASE_URL`, `RABBITMQ_URL`, `INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `STORAGE_SERVICE_URL`, `CATALOG_SERVICE_URL`, `PLATFORM_TENANT_ID`, `TEMA_HERENCIA_TTL_MS`; fail-fast sin `DATABASE_URL`/`RABBITMQ_URL`; cierre ordenado |

### 2. Implementación destino

- Crate en la raíz de `packages/stores-service/`, binario `stores-service`; el
  TypeScript y su workspace npm se retiran en la conmutación.
- `axum` + `tokio` para HTTP, `sqlx` para PostgreSQL, `lapin` para RabbitMQ,
  `reqwest` para identity/storage/catalog, `serde`/`serde_json`, `uuid`,
  `chrono`, `thiserror` y logs JSON propios.
- Capas: `http/` (router, envelope, rutas públicas e internas), `tiendas/`
  (dominio, tema, contenido, fondos, slugs), `ofertas/` (dominio y
  proyecciones públicas), `clients.rs` (herencia y validaciones),
  `consumidor.rs`, `rabbit.rs`, `auth.rs`, `db.rs`, `config.rs`.
- Inyección explícita por `AppState` con `Arc` (enmienda R-MS-3), igual que
  ADR-0004/0006/0008/0010.

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` y
   `cargo test` verdes.
2. Pruebas PostgreSQL (`tests/stores_pg.rs`): creación con slug, colisión y
   redirección de slug anterior; herencia de tema/campos por defecto; fondo y
   contenido; asignación/compensación de vendedor; publicación de oferta,
   histórico y cambio de margen; sincronización de stock idempotente.
3. Pruebas HTTP (`tests/http_stores.rs`): envelope, guards, validación,
   `403`/`404`, CSS del tema y rutas internas con clave.
4. Los OpenAPI versionados no cambian; el binario sirve el público en
   `/docs-json` y una prueba falla si el `x-servicio` no coincide.
5. Cobertura Rust en el job `rust` de CI (`cargo-llvm-cov`), con gate medido y
   redondeado a la baja con margen de 1 punto.
6. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR y el índice | Decisión registrada |
| 2 | Andamiaje Rust: configuración, HTTP, envelope, errores, métricas, auth y clientes | `cargo test` verde |
| 3 | Dominio de tiendas, tema, contenido, fondos, slugs y ofertas | Integración PostgreSQL verde |
| 4 | Consumidor `stores.stock`, pruebas HTTP, Dockerfile, job `rust` y cobertura | Build y gates verdes |
| 5 | Conmutación: retirar TypeScript, actualizar scripts, compose, CI y docs | `npm run build`, `npm run test:integration` y smoke verdes |

## Consecuencias

### Positivas

- Un servicio NestJS menos y menor consumo en reposo.
- Reglas de slug, tema y contenido con tipos estrictos y pruebas de paridad.
- Reutiliza el patrón y los gates de ADR-0004/0006/0008/0010.

### Costes y riesgos

- La herencia de tema depende de `identity-service`; la degradación a defaults
  debe conservarse para no romper el storefront.
- La validación de media depende de `storage-service`; un `404` y una caída se
  traducen a códigos distintos y deben conservarse.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  integración PostgreSQL en verde.
- Mismos slugs, redirecciones, herencia visual, contenido, ofertas y stock.
- OpenAPI, salud, métricas y rutas internas conservan el contrato.
- Smoke de `staging` verde en la primera promoción.

## Estado de implementación

En curso. El detalle de commits y verificaciones se registra en el PR final de
la migración.
