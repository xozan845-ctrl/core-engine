# ADR-0020: Contrato v2 de items sin `precio` ni `stock`

- **Estado**: Aceptado — fase 1 implementada
- **Fecha**: 2026-11-16
- **Ámbito**: `item-service`, `pricing-service`, `inventory-service`, `stores-service`, `orders-service`
- **Relacionado**: [ADR-0018](./0018-catalog-split-item-pricing-inventory.md), R-CD-5, R-AR-3

## Contexto

ADR-0018 separó el catálogo maestro en tres servicios, pero `item.items` conserva
`precio_cents` y `stock` como proyecciones del contrato de items. Esas columnas
permiten que `GET /api/v2/catalog/items` siga devolviendo `precio` y `stock` sin
romper al frontend, pero mantienen acoplados a `item-service` dos datos que ya
tienen dueño:

- el precio base lo resuelve `pricing-service` (listas, escalas, tarifas y
  promociones);
- el stock por ubicación lo posee `inventory-service` (y mantiene la proyección
  total por trigger).

Mientras el payload de items incluya ambos campos, cualquier cambio de precio o
stock obliga a escribir en el esquema de items y a mantener sincronizadas dos
fuentes. El contrato v1 también impide retirar las columnas y cierra el paso a
un modelo donde cada servicio sea la única fuente de su dato.

## Decisión

1. **Contrato interno v2** (servicio→servicio), sin `precio` ni `stock`:
   - `GET /internal/v2/items/lote?skus=...`
   - `GET /internal/v2/items/:id`
   `:id` devuelve la metadata completa del item (identidad, tipo,
   `track_inventory`, `track_serial`, atributos, estado, etc.); `lote` devuelve
   la metadata de disponibilidad por SKU (`empresa_id`, `sku`, `track_serial`,
   `estado`). Ninguno incluye `precio`, `stock` ni `seriales_disponibles`. Los
   endpoints `/internal/items/**` v1 se retiraron al migrar stores y orders.
2. **Precio**: `stores-service` y cualquier consumidor nuevo resuelven el precio
   base con `GET /internal/items/:id/precio` de `pricing-service`.
3. **Stock**: los consumidores resuelven stock y series disponibles con
   `POST /internal/inventario/stock` de `inventory-service`, que devuelve
   `{empresa_id, sku, stock, track_serial, seriales_disponibles, estado}` por
   SKU. El stock se calcula desde `inventory.item_stock_ubicacion` y usa
   `item.items.stock` solo como fallback cuando el item aún no tiene desglose.
4. **Contrato público**: `precio`, `precio_base`, `precio_desde`, `stock` y
   `stock_total` se retiraron de las respuestas de items, productos, variantes,
   servicios y combos. No hay frontend asociado, así que no queda ventana de
   compatibilidad. Las columnas internas también se retiraron: el precio base
   vive en `pricing` y el stock solo en `inventory`.
5. **Consumidores internos migrados**: `stores-service` (oferta: metadata v2 +
   precio de pricing + stock de inventario) y `orders-service`
   (`items_lote`: metadata v2 + stock/series de inventario). Los dobles de
   prueba mantienen la misma interfaz, por lo que el dominio no cambia.

## Fases y criterio de salida

| Fase | Cambio | Estado |
|---|---|---|
| 1 | Endpoints v2, stock batch de inventario y migración de stores/orders | Implementada |
| 2 | Frontend consume precio/stock desde pricing/inventory | No aplica (sin frontend) |
| 3 | Retirar `precio`/`stock` del payload público | Implementada |
| 3b | Retirar las columnas internas de `item.items` | Implementada |
| 4 | Retirar `/internal/items/**` v1 | Implementada |

La fase 3 no tenía dependencia externa: no hay frontend. La fase 3b movió el
precio base a `pricing.lista_precio_items` (fila base con `lista_id NULL`) y el
stock inicial a `inventory.item_stock_ubicacion`; `item.items` ya no tiene
`precio_cents` ni `stock`. `item_service` delega ambos al crear/editar items,
productos, variantes y combos.

## Consecuencias

- **Positivas**: cada dato tiene un dueño; se eliminan escrituras de
  proyecciones a medio plazo; los consumidores nuevos no dependen de columnas
  transitorias.
- **Coste**: una llamada más por oferta/checkout (precio y stock), compensable
  con batch en checkout. Los errores de pricing/inventario son fail-closed en
  stores/orders, igual que antes con el catálogo.
- **Rollback**: el código anterior requiere una migración inversa que recree
  `item.items.precio_cents/stock` desde pricing/inventory; no es automática.
  El contrato público no cambia (los campos ya no se exponían desde v1.9.0).
