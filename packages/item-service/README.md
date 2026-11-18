# item-service · catálogo maestro de items

`item-service` es la **autoridad del catálogo maestro**. Posee los ítems que una
empresa vende, presta o alquila, con sus atributos, variantes, imágenes,
composición (kits/recetas), unidades serializadas (IMEI/serial) y combos.

No posee precios ni stock: desde
[ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md) el
`catalog-service` se dividió en `item-service`, `pricing-service` e
`inventory-service`. El precio se resuelve en `pricing` y la disponibilidad en
`inventory`; `item` los proyecta para la lectura pública.

- [ADR-0016 · catalog-service en Rust](../../docs/adr/0016-catalog-service-en-rust.md)
- [ADR-0018 · split item/pricing/inventory](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)
- [ADR-0020 · contrato v2 de items](../../docs/adr/0020-contrato-v2-items.md)
- Contrato: [`docs/openapi/item-service/`](../../docs/openapi/item-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `item-service` |
| Stack | `axum` 0.8, `sqlx`, `jsonschema`, `lapin`, `reqwest` |
| Puerto | `3015` (`PORT`) |
| Esquema | `item` |
| Outbox | `item.outbox` (`OUTBOX_TABLA`) |
| Capacidades | `catalogo_items`, `codigo_barras` |
| Consumidores | `stores` (ofertas), `orders` (checkout), `inventory`, `pricing`, `analytics` |
| Dependencias internas | `identity` (capacidades/tenant), `pricing` (precio base), `inventory` (stock inicial), `storage` (imágenes), `stores` (ofertas activas) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | Sí para outbox | — | Broker |
| `OUTBOX_TABLA` | No | `item.outbox` | Outbox |
| `PORT` | No | `3015` | Puerto |
| `SERVICIO` | No | `item-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Rutas internas |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Capacidades |
| `STORAGE_SERVICE_URL` | No | `http://storage-service:3010` | Validar imágenes |
| `PRICING_SERVICE_URL` | No | `http://pricing-service:3014` | Precio base |
| `INVENTORY_SERVICE_URL` | No | `http://inventory-service:3013` | Stock inicial |
| `STORES_SERVICE_URL` | No | `http://stores-service:3003` | Ofertas activas |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout de clientes internos |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `items/mod.rs` | Item unificado (`producto`, `servicio`, `alquiler`), columnas, CRUD base |
| `productos/` | Productos por empresa, variantes, imágenes, metadatos, importación |
| `servicios/mod.rs` | Servicios con duración/costo; precio delegado a pricing |
| `series/mod.rs` | Unidades serializadas (IMEI/serial), estados, garantía |
| `componentes/mod.rs` | Composición de kits/recetas/combos (grafo acíclico) |
| `operaciones/mod.rs` | Importación masiva, escaneo por código y combos |
| `proyecciones.rs` | Clientes internos a pricing/inventory para precio y stock |
| `http/` | Router, handlers, envelope, validación tipo `class-validator` |
| `auth.rs` | Contexto, permisos (`catalogo.leer`/`catalogo.escribir`), capacidades |
| `outbox.rs` / `rabbit.rs` | Infraestructura de eventos |

**Nota de estado:** el outbox está implementado y corriendo, pero hoy el
servicio **no inserta eventos de negocio** en él (solo quedó la infraestructura
para futuras proyecciones). El catálogo se sincroniza por llamadas internas de
lectura (`/internal/v2/items/**`), no por eventos.

---

## 3. Modelo de datos

### `item.items` (tabla raíz)

| Columna | Tipo | Notas |
|---|---|---|
| `id` | uuid | PK |
| `empresa_id` | uuid | Tenant |
| `negocio_id` | uuid? | Negocio (ADR-0021) |
| `tipo` | text | `producto`, `servicio`, `alquiler` |
| `codigo` | text | SKU/código interno |
| `nombre`, `descripcion`, `categoria`, `marca` | text | |
| `costo_cents` | bigint | Coste entero en centavos |
| `track_inventory` | bool | Si inventory gestiona stock |
| `track_serial` | bool | Si exige serie/IMEI |
| `duration_minutes` | int? | Solo servicios (máx. 1440) |
| `atributos` | jsonb | Validado contra JSON Schema por categoría |
| `parent_item_id` | uuid? | Variante → item padre |
| `codigo_barras` | text? | Escaneo |
| `unidad`, `permite_fracciones`, `unidad_compra`, `contenido_cantidad/unidad` | | Presentación |
| `tipo_precio`, `tipo_costo` | text | Estrategia |
| `stock_minimo`, `stock_maximo` | int? | Umbrales |
| `garantia_meses` | int? | Base para series |
| `es_materia_prima` | bool | Insumos |
| `ubicacion` | text? | Ubicación física |
| `estado` | text | `activo`/`inactivo`/eliminado lógico |
| `eliminado_en` | timestamptz? | Baja lógica |
| `creado_en`, `actualizado_en` | timestamptz | |

### Tablas satélite

| Tabla | Contenido |
|---|---|
| `item.item_atributo_esquemas` | JSON Schema de atributos por empresa/categoría |
| `item.item_imagenes` | Imágenes (objeto de storage, orden, principal) |
| `item.item_componentes` | Composición: `item_id` → `componente_item_id`, `cantidad` |
| `item.item_series` | Una fila por unidad serializada: `serial`, `imei`, `estado`, `order_id`, `garantia_hasta` |
| `item.historico_precios` | Histórico de precio base proyectado |

### Invariantes

- Un **producto con variantes** no debe venderse como padre: las variantes son
  items hijos (`parent_item_id`) y el padre agrega stock/precio mínimo.
- Máximo **3 imágenes** por item y una sola principal.
- Máximo **50 variantes** por producto.
- Los **servicios** tienen `duracion_minutos` entre 1 y 1440.
- Los **combos** requieren al menos un componente.
- La composición es **acíclica**, del mismo tenant y solo admite productos.
- `serial`/`imei` son únicos por empresa; una unidad no puede estar en dos
  estados a la vez.
- Un item con `track_inventory=true` no acepta stock editado directamente en
  `item`: el stock es de `inventory` (error `STOCK_GESTIONADO_POR_INVENTARIO`).

---

## 4. Reglas de negocio

| Regla | Comportamiento |
|---|---|
| Tipos permitidos | Solo `producto`, `servicio`, `alquiler` |
| Atributos | Se validan contra el JSON Schema activo de la categoría; error `ATRIBUTOS_INVALIDOS` |
| Código | Único por empresa; normalizado; error `CODIGO_INVALIDO`/`CODIGO_REQUERIDO` |
| Precio | Se envía a `pricing` (`PUT /internal/items/{id}/precio-base`); si pricing falla, la operación falla de forma controlada |
| Stock inicial | Se registra en `inventory` (`POST /internal/inventario/stock/inicial`) |
| Imágenes | Deben existir en storage, ser públicas, del tenant y de tipo imagen |
| Escaneo | Busca por código de barras o código interno y devuelve producto/servicio |
| Importación | Máximo **200 filas**; valida y reporta válidas/omitidas; no hace *upsert* silencioso |
| Series | Al vender: `disponible → vendido` y calcula `garantia_hasta`; al devolver: `vendido → devuelto` |
| Serialización | Activar `track_serial` exige que el item aplique (`SERIALIZACION_NO_APLICA`) |
| Ofertas activas | No se permite borrar un item con ofertas activas (`OFERTAS_ACTIVAS`) |
| Baja | Lógica (`eliminado_en`), nunca física si tiene historia |

---

## 5. API pública

Todas las rutas requieren JWT con rol de catálogo (`super_admin`, `admin`,
`empresario` por gateway) y permiso `catalogo.leer`/`catalogo.escribir` en el
servicio. La capacidad `catalogo_items` debe estar activa.

| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/v2/catalog/items` | Listar items unificados (paginado, filtros por tipo/estado/categoría) |
| POST | `/api/v2/catalog/items` | Crear item (producto/servicio/alquiler) |
| POST | `/api/v2/catalog/items/alquileres` | Crear item de alquiler |
| GET | `/api/v2/catalog/items/{id}` | Detalle unificado con proyecciones de precio/stock |
| PATCH | `/api/v2/catalog/items/{id}` | Editar item |
| DELETE | `/api/v2/catalog/items/{id}` | Baja lógica |
| POST | `/api/v2/catalog/items/{id}/variantes` | Agregar variante |
| PATCH/DELETE | `/api/v2/catalog/items/{id}/variantes/{varianteId}` | Editar/eliminar variante |
| POST | `/api/v2/catalog/items/{id}/imagenes` | Agregar imagen (máx. 3) |
| DELETE | `/api/v2/catalog/items/{id}/imagenes/{imagenId}` | Eliminar imagen |
| GET/POST | `/api/v2/catalog/items/atributos/esquemas` | Listar/registrar JSON Schema |
| GET/POST | `/api/v2/catalog/items/{id}/componentes` | Listar/agregar componente |
| PATCH/DELETE | `/api/v2/catalog/items/{id}/componentes/{componenteId}` | Editar cantidad/quitar |
| GET/POST | `/api/v2/catalog/items/{id}/series` | Listar/registrar unidad serializada |
| PATCH | `/api/v2/catalog/items/{id}/serializacion` | Activar/desactivar serialización |
| PATCH/GET | `/api/v2/catalog/series/{serieId}` / `/{serial}` | Actualizar unidad / consultar por serial y garantía |
| POST | `/api/v2/catalog/items/importar` | Importar productos (máx. 200 filas) |
| GET | `/api/v2/catalog/escanear` | Buscar por código/barras |
| GET/POST | `/api/v2/catalog/combos` | Listar/crear combo |
| GET/PATCH/DELETE | `/api/v2/catalog/combos/{id}` | Detalle/editar/baja |

### Proyecciones de lectura

El detalle y listado público enriquecen el item con `precio` y `stock`
obtenidos de `pricing` e `inventory`. Si un servicio de proyección no responde,
la lectura devuelve el item sin ese campo o con error controlado según el
endpoint; nunca inventa valores.

---

## 6. API interna (contrato v2)

Protegida con `x-internal-key`. **No incluye precio ni stock** (ADR-0020):

| Método | Ruta | Respuesta |
|---|---|---|
| GET | `/internal/v2/items/lote?ids=a,b,c` | Items por lote, sin `precio`, `stock` ni `seriales_disponibles` |
| GET | `/internal/v2/items/{id}` | Item por id con el mismo recorte |

Consumidores: `orders` (resolución de líneas), `stores` (publicación de
ofertas), `inventory`/`pricing` (metadatos), `analytics`.

---

## 7. Seguridad y multi-tenancy

- Cada consulta filtra por `empresa_id` (y `negocio_id` cuando aplica).
- `EMPRESA_NO_AUTORIZADA` si se intenta operar fuera del tenant del JWT.
- El item de otra empresa no se revela: se responde `NO_ENCONTRADO`.
- La validación de imágenes consulta storage y exige `publico=true`,
  `estado=activo`, mismo tenant y MIME de imagen.
- Los permisos finos (`catalogo.leer`, `catalogo.escribir`) se comprueban
  además del rol global.
- Los errores de validación replican el formato
  `VALIDACION` + `detalles[{campo, restricciones}]`.

---

## 8. Observabilidad y operación

- `GET /health`, `GET /metrics`, `GET /docs`, `GET /docs-json`.
- Métricas comunes + outbox (`pending`, `published_total`, `failures_total`).
- Logs JSON con `x-request-id`; operaciones de importación registran el
  resumen.
- Apagado ordenado: aborta outbox y cierra pool.
- Sin consumidores de eventos ni workers de negocio.

---

## 9. Pruebas

```bash
cd packages/item-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/catalog_pg.rs`: CRUD, variantes, imágenes, componentes y series contra
  PostgreSQL real.
- `tests/http_catalog.rs`: contrato HTTP, validación y permisos.
- `tests/schema.sql`: esquema mínimo para las suites.
- `productos/tests.rs`: reglas puras de metadatos y márgenes.

---

## 10. Referencias

- [ADR-0016](../../docs/adr/0016-catalog-service-en-rust.md)
- [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)
- [ADR-0020](../../docs/adr/0020-contrato-v2-items.md)
- [Contrato público](../../docs/openapi/item-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/item-service/internal.openapi.json)
- [README de paquetes](../README.md)
