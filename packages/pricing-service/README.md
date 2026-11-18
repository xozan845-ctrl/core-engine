# pricing-service · motor de precios

`pricing-service` es la **autoridad de precios** de Core Engine. Resuelve el
precio unitario de un item según cantidad, lista de precio, segmento o rango de
fechas, y administra listas, escalas, tarifas por temporada y promociones.

No posee stock ni el catálogo maestro: es la tercera pieza del split de
`catalog-service` ([ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)).
Los precios base viven aquí (no en `item.items`) desde ADR-0020 fase 3b, y
mantienen histórico.

- [ADR-0018 · split del catálogo](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)
- [ADR-0020 · contrato v2 de items](../../docs/adr/0020-contrato-v2-items.md)
- Contrato: [`docs/openapi/pricing-service/`](../../docs/openapi/pricing-service/)

---

## 1. Ficha técnica

| Campo | Valor |
|---|---|
| Crate | `pricing-service` |
| Stack | `axum` 0.8, `sqlx`, `lapin`, `reqwest` |
| Puerto | `3014` (`PORT`) |
| Esquema | `pricing` (lee `item.items`) |
| Outbox | `pricing.outbox` (infraestructura; sin eventos de negocio hoy) |
| Capacidades | `catalogo_items` |
| Dependencias | `identity` (capacidades/tenant), `item` (existencia/tipo de item) |
| Consumidores | `orders`, `stores`, `commissions` (vía API interna) |
| Expuesto al host | No |

### Variables de entorno

| Variable | Obligatoria | Default | Uso |
|---|---|---|---|
| `DATABASE_URL` | **Sí** | — | PostgreSQL |
| `RABBITMQ_URL` | Sí para outbox | — | Broker |
| `OUTBOX_TABLA` | No | `pricing.outbox` | Outbox |
| `PORT` | No | `3014` | Puerto |
| `SERVICIO` | No | `pricing-service` | Etiqueta |
| `INTERNAL_API_KEY` | Recomendada | — | Rutas internas |
| `IDENTITY_SERVICE_URL` | No | `http://identity-service:3001` | Capacidades |
| `ITEM_SERVICE_URL` | No | `http://item-service:3015` | Item/metadatos |
| `INTERNAL_REQUEST_TIMEOUT_MS` | No | `3000` | Timeout interno |

---

## 2. Arquitectura interna

| Módulo | Responsabilidad |
|---|---|
| `precios/mod.rs` | Listas, segmentos, precios por rango de cantidad, escalas base y resolución |
| `tarifas/mod.rs` | Tarifas por fecha y cotización por rango (máx. 366 días) |
| `promociones/mod.rs` | 2x1, descuento por cantidad, porcentaje y regalo |
| `money.rs` | Enteros en centavos y validación de rangos |
| `http/` | Listas, precios, escalas, tarifas, promociones e internos |
| `auth.rs` | Roles/permisos y capacidades |

---

## 3. Modelo de datos

| Tabla | Contenido |
|---|---|
| `pricing.listas_precio` | `codigo`, `nombre`, `moneda`, `segmento_codigo`, `predeterminada`, `activo` |
| `pricing.lista_precio_items` | Precio por item y rango de cantidad; `lista_id NULL` = **escala base** del item |
| `pricing.item_tarifas` | Precio por rango de fechas (`desde`, `hasta`, `precio_cents`) |
| `pricing.promociones` | Tipo, item/SKU, cantidades, porcentaje, vigencia y estado |
| `pricing.historico_precios_base` | Auditoría de cambios de precio base |
| `pricing.outbox` | Infraestructura de eventos |

### Invariantes

- Montos siempre **enteros en centavos**; nunca flotantes persistidos.
- `desde_cantidad >= 1`; `hasta_cantidad` nulo significa “sin límite”.
- Las escalas de un mismo item/lista **no se solapan**
  (`La escala se solapa con otra del mismo item`).
- Una lista es única por `codigo` y por segmento activo.
- Una sola lista **predeterminada** por empresa.
- Las tarifas de un item **no se solapan** entre fechas.
- `fecha_hasta >= fecha_desde`; un rango de cotización no supera **366 días**.
- El precio base es la fila `lista_id = NULL AND desde_cantidad = 1`.
- Cambiar el precio base **siempre** escribe histórico, aunque el valor no
  cambie (trazabilidad).

---

## 4. Resolución de precio

`GET /api/v2/catalog/items/{id}/precio?cantidad=N[&lista_codigo=…][&segmento_codigo=…]`

Algoritmo:

```
1. Validar item (empresa, no eliminado) y cantidad (1..1.000.000)
2. Resolver lista:
   a. lista_codigo explícita (debe existir y estar activa)
   b. segmento_codigo (lista activa del segmento)
   c. lista predeterminada de la empresa
   d. ninguna → se usa la escala base (lista_id NULL)
3. Buscar la escala aplicable:
   - lista resuelta → precio de esa lista para la cantidad
   - sin lista → escala base del item para la cantidad
4. Origen del precio: "lista" | "escala" | "base"
5. total = unitario × cantidad
```

Respuesta:

```json
{
  "item_id": "…",
  "cantidad": 12,
  "lista": { "id": "…", "codigo": "mayorista", "segmento_codigo": "mayorista" },
  "origen": "lista",
  "escala": { "desde_cantidad": 10, "hasta_cantidad": 49, "precio_cents": 900 },
  "precio_unitario_cents": 900,
  "precio_total_cents": 10800
}
```

> Es una **lectura**: no reserva stock ni crea órdenes. `orders` y `stores`
> pueden degradar al precio de la oferta si pricing no responde.

### Cotización por fechas

`GET /api/v2/catalog/items/{id}/cotizacion?desde=YYYY-MM-DD&hasta=YYYY-MM-DD`

- Agrupa días consecutivos con la misma tarifa; los días sin tarifa usan el
  precio base.
- Devuelve `desglose` y `total_cents`.
- Rango máximo 366 días; fechas invertidas → `RANGO_INVALIDO`.
- Pensado para alquiler/hotel; la disponibilidad de recursos es de
  `scheduling`.

---

## 5. Promociones

| Tipo | Campos requeridos | Efecto |
|---|---|---|
| `2x1` | `cantidad_requerida=2` (por defecto) | Lleva 2, paga 1 |
| `descuento_cantidad` | `cantidad_requerida >= 1` | Descuento al alcanzar cantidad |
| `descuento_porcentaje` | `descuento_porcentaje` 1–100 | Descuento porcentual |
| `regalo` | `cantidad_requerida >= 1`, `cantidad_gratis >= 1` | Unidades gratis |

- Referencia el item por `item_id` y/o `sku`; debe pertenecer a la empresa.
- Vigencia obligatoria y coherente (`VIGENCIA_INVALIDA`).
- Estado `activo`/`inactivo`; la baja es lógica.
- La promoción **no** modifica el precio base ni posee stock: solo describe la
  regla comercial que consumen `orders`/`stores`.

---

## 6. API pública

Requiere capacidad `catalogo_items` y rol de precios.

| Método | Ruta | Descripción |
|---|---|---|
| GET/POST | `/api/v2/catalog/listas-precio` | Listar/crear lista o segmento |
| PATCH | `/api/v2/catalog/listas-precio/{listaId}` | Actualizar lista |
| GET/POST | `/api/v2/catalog/listas-precio/{listaId}/precios` | Listar/agregar precio o rango |
| PATCH/DELETE | `/api/v2/catalog/listas-precio/{listaId}/precios/{precioId}` | Editar/eliminar precio |
| GET | `/api/v2/catalog/items/{id}/precio` | Resolver precio unitario |
| GET/POST | `/api/v2/catalog/items/{id}/escalas` | Listar/agregar escala base |
| PATCH/DELETE | `/api/v2/catalog/items/{id}/escalas/{escalaId}` | Editar/eliminar escala |
| GET/POST | `/api/v2/catalog/items/{id}/tarifas` | Listar/crear tarifa por fechas |
| PATCH/DELETE | `/api/v2/catalog/items/{id}/tarifas/{tarifaId}` | Editar/eliminar tarifa |
| GET | `/api/v2/catalog/items/{id}/cotizacion` | Cotizar por rango |
| GET/POST | `/api/v2/catalog/promociones` | Listar/crear promoción |
| GET/PATCH/DELETE | `/api/v2/catalog/promociones/{id}` | Detalle/editar/baja |

---

## 7. API interna (`x-internal-key`)

| Método | Ruta | Uso |
|---|---|---|
| GET | `/internal/items/{id}/precio?cantidad=N[&lista_codigo&segmento_codigo]` | Resolver precio (orders/stores) |
| POST | `/internal/items/precios` | Resolver precios por lote |
| GET | `/internal/items/{id}/cotizacion?desde&hasta` | Cotización por fechas |
| PUT | `/internal/items/{id}/precio-base` | Fijar precio base (item-service) |

`precios` por lote acepta:

```json
{
  "empresa_id": "uuid",
  "lista_codigo": "mayorista",
  "segmento_codigo": null,
  "lineas": [{ "clave": "l1", "item_id": "uuid", "cantidad": 3 }]
}
```

y responde `{precios:[{clave, origen, precio_unitario_cents, precio_total_cents}]}`.

---

## 8. Eventos

Hoy `pricing` **no publica eventos de negocio**: el outbox está desplegado y el
publicador corre, pero ninguna operación inserta filas. Los cambios de precio
no se propagan por bus; se consultan por API interna y `item-service` mantiene
una proyección de lectura.

Esto es deliberado: un precio es una lectura barata y fuertemente consistente
para el checkout, no un hecho que deba replicarse de forma eventual.

---

## 9. Seguridad y multi-tenancy

- Filtro por `empresa_id` en todas las tablas.
- `EMPRESA_NO_AUTORIZADA` si se intenta operar otro tenant.
- El item se valida contra `pricing`/`item` antes de aceptar un precio.
- Permisos de catálogo/precios en el servicio, además del rol global.
- Los internos exigen `x-internal-key` en tiempo constante.
- El histórico de precios base no se borra: es auditoría (RN-08).

---

## 10. Observabilidad y operación

- `/health`, `/metrics`, `/docs`, `/docs-json`.
- Métricas HTTP comunes y de outbox.
- Logs JSON correlacionados.
- Worker de outbox cada segundo; sin consumidores ni schedulers.
- Apagado ordenado.

---

## 11. Pruebas

```bash
cd packages/pricing-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `tests/pricing_pg.rs`: listas, escalas, solapamientos, resolución, tarifas,
  promociones y precio base con histórico.
- `tests/common/` + `tests/schema.sql`: esquema real.
- Casos puros: validación de código/segmento, rangos de cantidad y dinero.

---

## 12. Referencias

- [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md)
- [ADR-0020](../../docs/adr/0020-contrato-v2-items.md)
- [Contrato público](../../docs/openapi/pricing-service/public.openapi.json)
- [Contrato interno](../../docs/openapi/pricing-service/internal.openapi.json)
- [README de paquetes](../README.md)
