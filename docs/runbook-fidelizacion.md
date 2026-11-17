# Runbook — fidelización

Complementa [ADR-0019](./adr/0019-fidelizacion-por-capacidades.md). La capacidad
`fidelizacion` vive en `orders-service` y se activa por configuración (vertical,
pack o add-on). No requiere desplegar servicios nuevos.

## Activación

1. **Al crear la empresa** (recomendado): enviar
   `capacidades_extra: ["fidelizacion"]` en `POST /api/v2/empresas`. El alta
   valida que la capacidad esté disponible en la vertical y la activa en la
   misma transacción.
2. **Después del alta**:
   `PATCH /api/v2/empresas/:id/capacidades/fidelizacion` con
   `{"activo": true}`.
3. **Verificación**: `GET /api/v2/empresas/:id/features` debe incluir
   `fidelizacion.programas`, `fidelizacion.tarjetas` y
   `fidelizacion.canjes`.

La capacidad viene **inactiva por defecto** en todas las verticales y packs; si
no se activa, ninguna empresa existente cambia de comportamiento.

## Configuración

1. **Programa** (`PUT /api/v2/fidelizacion/programa`):
   - `puntos_por_unidad`: puntos por cada unidad de moneda (100 centavos).
   - `expira_meses`: opcional; se guarda para reglas de expiración futuras.
   - `activo`: si se desactiva, no se acumulan puntos nuevos.
2. **Tarjetas** (`POST /api/v2/fidelizacion/tarjetas`): una por parte
   (`identity.partes`). El código se autogenera (`FID-XXXXXXXX`) o se envía
   explícito.
3. **Recompensas** (`POST /api/v2/fidelizacion/recompensas`): costo en puntos y
   `stock` opcional (`null` = ilimitado).

## Operación

- **Acumulación**: automática al pasar la orden a `pagada` (pago automático,
  confirmación manual o webhook). Se calcula por empresa a partir de los ítems y
  solo si la capacidad está activa.
- **Idempotencia**: índice único parcial `(tarjeta_id, order_id)` para
  `tipo = 'acumulacion'`. Reintentos y replays no duplican puntos.
- **Ajustes**: `POST /api/v2/fidelizacion/tarjetas/:id/ajustes` con `puntos`
  positivo o negativo. Nunca deja saldo negativo.
- **Canjes**: `POST /api/v2/fidelizacion/canjes` por `tarjeta_id`/`codigo_tarjeta`
  y `recompensa_id`/`recompensa_codigo`. Valida tarjeta activa, saldo y stock, y
  descuenta todo en una transacción.
- **Expiración**: un job cada 6 h expira los puntos de tarjetas activas sin
  movimientos durante `expira_meses`. Registra un movimiento `expiracion` y deja
  el saldo en 0; es idempotente. `expira_meses = null` desactiva la expiración.
- **Comprador**: `GET /api/v2/fidelizacion/mis-tarjetas` y
  `GET /api/v2/fidelizacion/mis-tarjetas/:id/movimientos`. La propiedad se
  deriva de las órdenes propias (`orden_vista.comprador_id`).

## Diagnóstico

| Síntoma | Causa probable | Acción |
|---|---|---|
| `403 CAPACIDAD_NO_ACTIVA` | La empresa no tiene `fidelizacion` activa | Activar por `PATCH /capacidades/fidelizacion` o `capacidades_extra` |
| La orden se pagó y no hay puntos | Sin `cliente_parte_id`, sin tarjeta o programa inactivo | Verificar parte, tarjeta y `programa.activo` |
| No se acumularon puntos y hay log `fidelizacion` | `identity-service` no disponible o error de BD | Revisar logs; reintentar (idempotente) |
| `PROGRAMA_NO_CONFIGURADO` | No existe programa activo al emitir tarjeta | Crear el programa primero |
| `PUNTOS_INSUFICIENTES` | Saldo menor al costo o ajuste negativo | Revisar movimientos de la tarjeta |
| `RECOMPENSA_SIN_STOCK` | `stock = 0` | Reponer stock o dejar `stock = null` |

Consultas útiles:

```sql
-- saldo y estado de tarjetas
SELECT codigo, parte_id, puntos, estado FROM orders.fidelizacion_tarjetas
 WHERE empresa_id = '<empresa>';

-- movimientos de una tarjeta
SELECT tipo, puntos, saldo_tras, order_id, creado_en
  FROM orders.fidelizacion_movimientos
 WHERE tarjeta_id = '<tarjeta>' ORDER BY creado_en DESC;

-- acumulaciones por orden (idempotencia)
SELECT tarjeta_id, order_id, puntos FROM orders.fidelizacion_movimientos
 WHERE tipo = 'acumulacion' ORDER BY creado_en DESC LIMIT 20;
```

## Rollback

- **Desactivar la capacidad**: `PATCH /api/v2/empresas/:id/capacidades/fidelizacion`
  con `{"activo": false}`. Las rutas de empresa devuelven `CAPACIDAD_NO_ACTIVA`;
  los datos permanecen.
- **Desactivar acumulación**: `PUT /programa` con `{"activo": false}`. No se
  acumulan puntos nuevos; los canjes siguen disponibles si la capacidad está
  activa.
- **Detener la expiración**: `PUT /programa` con `{"expira_meses": null}`.
- **No borrar tablas**: la migración 87 es aditiva y los datos son historial de
  puntos; un rollback destructivo requiere decisión explícita y respaldo.
