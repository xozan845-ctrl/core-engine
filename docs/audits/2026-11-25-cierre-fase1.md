# Cierre de Fase 1 — propiedad de datos y readiness

- **Fecha:** 2026-11-25
- **Base:** `develop` @ `72161da7` + este PR.
- **Alcance:** los 4 hallazgos P1 de la auditoría de contrato de servicio.
- **Método:** ejecución de tests unitarios e integración PostgreSQL de los
  servicios afectados + suite `services-contract` + revisión de SQL.

## 1. Resultado

| Hallazgo original | Estado | Evidencia |
|---|---|---|
| `inventory` escribía `item.item_series` | ✅ eliminado | `item.marcar_series_vendidas/liberar/devolver`; `cross-schema-write` vacío |
| `pricing` escribía `item.items.tipo_precio` | ✅ eliminado | `item.marcar_tipo_precio` |
| `identity` insertaba `inventory.ubicaciones` | ✅ eliminado | `inventory.crear_ubicacion_predeterminada` |
| `identity` actualizaba `finance.configuracion_fiscal` | ✅ eliminado | `finance.actualizar_region_fiscal` |
| `/ready` en `orders`, `inventory`, `identity`, `finance` | ✅ añadido | `GET /ready` con `SELECT 1` y `503` si la BD no responde |

La deuda arquitectónica baja de **34 a 29** entradas:

- 4 `readiness:*:ready` retiradas.
- 1 `cross-schema:identity-service:inventory` retirada.
- Permanecen 11 `readiness` (servicios restantes) y 13 lecturas cross-schema
  agrupadas por servicio/esquema.

## 2. Diseño aplicado

Se usó la API de dominio transitoria del ADR-0051: funciones en el esquema dueño,
invocadas desde la misma transacción del llamador. No hay DML directo a tablas
ajenas y se preserva la atomicidad de:

- reserva de stock ↔ serie vendida;
- cancelación/devolución ↔ serie liberada/devuelta;
- alta de empresa/negocio ↔ ubicación principal;
- cambio de región ↔ configuración fiscal.

El check `cross-schema-write` es **duro** (no admite baseline). Las lecturas
cross-schema siguen siendo deuda P2 y se sustituirán por proyecciones/API en la
Fase 2.

## 3. Verificación ejecutada

| Comando / suite | Resultado |
|---|---|
| `cargo test --locked` (inventory, con PostgreSQL) | ✅ |
| `cargo test --locked` (pricing, con PostgreSQL) | ✅ |
| `cargo test --locked` (identity, migraciones reales) | ✅ 35 tests |
| `cargo test --locked` (finance) | ✅ 19 tests |
| `npm run test:integration` | ✅ |
| `npm run rules:check` | ✅ 24 suites / 101 tests |
| `npm run architecture:check` | ✅ deuda regenerada |
| `npm run baseline:guard` | ✅ sin retroceso |
| `npm run lint` | ✅ |

## 4. Pendiente (Fase 2)

1. `/ready` en los 11 servicios restantes.
2. Sustituir lecturas cross-schema por proyecciones locales o API interna:
   `identity→finance`, `item→identity/inventory`, `inventory→item/orders`,
   `pricing→item`, `stores→identity`, `orders→identity`,
   `commissions→orders/identity`, `finance→identity`, `analytics→identity`.
3. Mover las funciones de dominio a eventos/HTTP cuando exista un modelo local
   de reserva de series con bloqueo.
4. Añadir AsyncAPI y checks de las reglas críticas sin cobertura (R-AR-2..4,
   R-DI-2..5, R-DB-2..5).
