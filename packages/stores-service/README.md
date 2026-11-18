# stores-service · tiendas, storefront y ofertas

`stores-service` es la **autoridad del canal de venta**: la tienda de cada
vendedor, el storefront público (tema, contenido, SEO, medios), los slugs con
historial y las **ofertas** (item publicado con margen y precio final).

Es el puente entre el catálogo (`item`/`pricing`/`inventory`) y la compra: una
oferta referencia un item, fija el margen del vendedor y mantiene el stock
visible sincronizado por eventos.

- [ADR-0011 · stores-service en Rust](../../docs/adr/0011-stores-service-en-rust.md)
- ADR-0026 · slugs (histórico: [docs/decisiones.md](../../docs/decisiones.md))
- Contrato: [`docs/openapi/stores-service/`](../../docs/openapi/stores-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `stores-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest`, `unicode-normalization` |
| Puerto | `3003` (`PORT`) |
| Esquema | `stores` (lee `identity.negocios`/`asignaciones_personal`) |
| Outbox | No publica eventos (solo consume `stock.updated`) |
| Consume | `stock.updated` (cola `stores.stock`) |
| Capacidades | `catalogo_items` para ofertas; storefront público |
| Dependencias | `identity` (tema heredado), `storage` (medios), `item` (catálogo), `pricing` (precio base), `inventory` (stock) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | **Sí** | — | Consumidor de stock |
| `PORT` | No | `3003` | Puerto |
| `SERVICIO` | No | `stores-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Internos + llamadas salientes |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Herencia de tema |
| `STORAGE_SERVICE_URL` | No | `http://storage-service:3010` | Validar medios |
| `ITEM_SERVICE_URL` | No | `http://item-service:3015` | Catálogo |
| `PRICING_SERVICE_URL` | No | `http://pricing-service:3014` | Precio base |
| `INVENTORY_SERVICE_URL` | No | `http://inventory-service:3013` | Stock |
| `PLATFORM_TENANT_ID` | No | — | Tenant de plataforma |
| `TEMA_HERENCIA_TTL_MS` | No | `60000` | TTL de caché de herencia |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `tiendas/mod.rs` | Alta, resolución por UUID/slug, logo/fondos, personalización, contenido, asignación de vendedor |
| `tiendas/tema.rs` | Plantillas, paleta, herencia plataforma→empresa→tienda y CSS |
| `tiendas/personalizacion.rs` | Hero, SEO y overrides validados |
| `tiendas/contenido.rs` | Secciones, políticas, redes, anuncio |
| `tiendas/media.rs` | Fondo de tienda y móvil, póster, overlay, posición |
| `tiendas/slug.rs` | Normalización, reservados, colisiones, historial |
| `ofertas/mod.rs` | Publicación, margen RN-01, stock visible RN-02, histórico |
| `ofertas/publico.rs` | Proyección pública de oferta |
| `clients.rs` | identity/storage/item/pricing con caché TTL para tema |
| `consumidor.rs` | `stock.updated` → sincroniza ofertas |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `stores.tiendas` | Empresa, negocio, propietario, vendedor, participación, nombre, slug, logo, fondos, tema, hero, SEO, contenido |
| `stores.tienda_slugs` | Historial de slugs para redirección `301` |
| `stores.ofertas` | Item publicado: tipo, `item_id`, SKU, nombre, atributos, imagen, margen, precio base/venta, stock, estado, moneda |
| `stores.historico_precios` | Precio anterior/nuevo y vigencia (RN-08) |
| `stores.eventos_procesados` | Idempotencia de `stock.updated` |
| `stores.eventos_procesados` | Única tabla de eventos (idempotencia de consumo) |

### Invariantes

- Una tienda pertenece a **una empresa** y, si existe, a un negocio.
- Nombre de tienda: **2–100 caracteres**.
- Slug: 2–80, `[a-z0-9-]`, sin UUID, no reservado; colisiones con sufijo
  (máx. 3 intentos al generar, 1 si es explícito).
- Un vendedor no puede publicar dos veces el mismo item en la misma tienda.
- El item debe ser de la **misma empresa** y estar `activo`.
- Margen entre **0 y 90 %** (RN-01).
- Un item con `track_inventory` y stock `< 1` **no se publica** (RN-02).
- El stock de la oferta se sincroniza desde `stock.updated`; el precio base se
  congela al publicar y los cambios de margen aplican a compras nuevas.
- El borrado de tienda/oferta es lógico cuando tiene historia.

---

## 4. Reglas de negocio

| Regla | Comportamiento |
|---|---|
| RN-01 | `precio_venta = round(precio_base × (1 + margen/100))`, enteros en centavos |
| RN-02 | Publicación rechazada si stock < 1; el consumidor pasa la oferta a `agotada` cuando baja a 0 |
| RN-08 | Cambio de margen/precio base escribe histórico; no reescribe compras pasadas |
| Participación | La tienda guarda `participacion_porcentaje` del vendedor; se congela en la oferta |
| Propiedad | Solo el propietario o personal asignado a la tienda puede publicar/editar |
| Herencia visual | `plataforma → empresa → tienda`; cada nivel solo sobrescribe lo definido |
| Medios | Logo/fondo deben existir en storage, ser públicos, del tenant y del MIME permitido |
| Contenido | Máx. 12 secciones, 2000 chars “sobre nosotros”, 1000 por política, 6 redes, anuncio 120/300 |
| SEO | Título ≤ 70, descripción ≤ 300, imagen validada |
| Slug | Cambiar genera historial y `301` desde los anteriores |

---

## 5. Tienda y storefront

### Alta

`POST /api/v2/vendedores/tienda` crea la tienda del vendedor autenticado:

- Si es `empresario`, la participación se toma de su contexto; si es
  `vendedor`, se valida contra su asignación.
- Se asigna el negocio predeterminado si no se indica uno.
- Genera slug desde el nombre o usa el solicitado.
- Un vendedor con tienda existente recibe conflicto.

### Storefront público

`GET /api/v2/tiendas/{id}` acepta **UUID o slug** y devuelve la tienda con sus
ofertas públicas. Es una ruta pública (sin JWT) y solo expone datos publicables.

- `GET /api/v2/tiendas/{id}/tema.css` devuelve variables CSS:
  `--tienda-color-primario`, `--tienda-color-secundario`,
  `--tienda-color-acento`, `--tienda-color-fondo`, `--tienda-color-texto`,
  `--tienda-fuente`, `--tienda-radio-bordes`, `--tienda-modo`,
  `color-scheme`. `Content-Type: text/css`, `Cache-Control: public, max-age=60`.
- Un slug histórico responde `301` a la tienda vigente.

### Personalización

| Ruta | Contenido |
|---|---|
| `PATCH /tiendas/{id}/logo` | Logo público (objeto de storage) |
| `PATCH /tiendas/{id}/background` | Fondo desktop/móvil, póster, overlay 0–100, posición, alt, activo |
| `PATCH /tiendas/{id}/personalizacion` | Plantilla (`clasica`, `minimal`, `boutique`, `mercado`), tema, hero, SEO |
| `PATCH /tiendas/{id}/contenido` | Secciones, sobre nosotros, políticas, redes, anuncio |
| `PATCH /tiendas/{id}/slug` | Nuevo slug + historial |

Plantillas y fuentes: `system`, `inter`, `poppins`, `montserrat`, `playfair`.
Modos: `claro`, `oscuro`, `auto`.

---

## 6. Ofertas

| Método | Ruta | Descripción |
|---|---|---|
| POST | `/api/v2/vendedores/productos` | Publicar item con margen |
| PATCH | `/api/v2/vendedores/productos/{id}` | Cambiar margen de una oferta |
| GET | `/api/v2/vendedores/me/ofertas` | Ofertas del vendedor |
| GET | `/api/v2/empresarios/me/tiendas` | Tiendas de la empresa |
| GET | `/api/v2/vendedores/me/tienda` | Tienda propia |

La publicación consulta `item` (tipo, precio, stock, empresa) y `pricing`
(cuando aplica), calcula el precio final y guarda la oferta + histórico en una
transacción. El cambio de margen también escribe histórico.

---

## 7. API interna (`x-internal-key`)

| Método | Ruta | Uso |
|---|---|---|
| GET | `/internal/ofertas?ids=…` | Lote de ofertas para orders |
| GET | `/internal/ofertas/producto/{productoId}/activas` | Conteo de ofertas activas (item) |
| GET | `/internal/tiendas?ids=…` | Tiendas por lote |
| GET | `/internal/tiendas/empresa/{empresaId}` | Tiendas de una empresa |
| GET | `/internal/tiendas/vendedor/{vendedorId}` | Tienda del vendedor |
| GET | `/internal/tiendas/{id}` | Tienda por id |
| POST | `/internal/tiendas/{id}/asignar-vendedor` | Asignación (identity) |
| POST | `/internal/tiendas/{id}/desasignar-vendedor` | Compensación |

La respuesta interna de ofertas incluye precio base/venta, margen, stock,
estado, atributos e imagen; es la que usa el checkout.

---

## 8. Eventos

### Consume

| Evento | Cola | Efecto |
|---|---|---|
| `stock.updated` | `stores.stock` | Actualiza `stores.ofertas.stock` y estado `activa`/`agotada` por empresa/SKU |

Idempotencia por `event_id` en `stores.eventos_procesados`. Si el payload trae
varias empresas, solo se procesan las del evento; cada item se actualiza por
`(empresa_id, sku)`.

### Publica

Hoy **no se publican eventos de negocio**. La tienda y las ofertas se consultan
por API interna. La sincronización crítica (stock) llega desde inventory.

---

## 9. Seguridad y multi-tenancy

- Toda tienda/oferta filtra por `empresa_id` (y `negocio_id`).
- La tienda pública solo expone campos publicables; nunca datos fiscales ni
  identificadores internos.
- La publicación exige que el item sea de la misma empresa.
- Los medios se validan contra storage: tenant, estado activo, público, MIME.
- Roles: `vendedor`/`empresario` para su tienda; admin para administración.
- El gateway restringe `/api/v2/tiendas/{id}` y `tema.css` como públicos; el
  resto exige JWT.

---

## 10. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas HTTP comunes.
- Logs JSON correlacionados; eventos con `stores-event-<event_id>`.
- Caché TTL de herencia de tema (`TEMA_HERENCIA_TTL_MS`), con degradación a
  defaults si identity no responde (R-AR-7).
- Supervisor RabbitMQ + reintento cada 10 s; DLQ al agotar reintentos.
- Apagado ordenado.

---

## 11. Pruebas

```bash
cd packages/stores-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/stores_pg.rs`: tiendas, slugs, herencia, ofertas e histórico.
- `tests/http_stores.rs`: contrato HTTP, permisos y storefront público.
- `tests/common/`: utilidades de esquema.
- Casos puros: margen RN-01, slug, tema, contenido, medios.

---

## 12. Referencias

- [ADR-0011](../../docs/adr/0011-stores-service-en-rust.md)
- ADR-0026 · slugs (histórico: [docs/decisiones.md](../../docs/decisiones.md))
- [Contrato público](../../docs/openapi/stores-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/stores-service/internal.openapi.json)
- [README de paquetes](../README.md)
