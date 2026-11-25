# Cierre — venta directa y dependencia de `commissions-service`

- **Fecha:** 2026-11-25
- **Hallazgo original:** [auditoría de producción 2026-11-20](./2026-11-20-auditoria-produccion.md) §3.3
- **Resolución:** [ADR-0053](../adr/0053-cobro-centralizado-dispersion-diaria.md)

## Hallazgo

`pack_retail` incluye la capacidad `liquidaciones` (módulo `commissions`).
`commissions-service` no se desplegaba en producción, por lo que:

- `venta.directa.acreditada` no se emitía;
- `finance-service` no cerraba el pasivo/acreedor del empresario;
- la venta quedaba en “Fondos por liquidar”;
- las rutas de liquidaciones devolvían error/502.

## Resolución aplicada

Desde ADR-0053:

1. `docker-compose.yml` de **producción** define `commissions-service`
   (`image: .../core-engine-commissions-service:prod`).
2. El API Gateway enruta `/api/v2/admin/liquidaciones`,
   `/api/v2/empresarios/me/liquidaciones`, `/api/v2/vendedores/me/liquidaciones`
   y la dispersión diaria.
3. `/ready` de producción incluye `commissions` (12 servicios) y una prueba
   impide que vuelva a quedar fuera.
4. `commissions-service` consume `order.completado` y emite
   `venta.directa.acreditada` desde su outbox; `finance-service` lo consume y
   registra el asiento `2.1.2 Fondos por liquidar → 2.1.6 Liquidación al
   empresario` en `finance.ventas_lineas`.

## Comportamiento esperado

```text
order.completado
  └─> commissions-service: acredita línea directa
        └─> outbox: venta.directa.acreditada
              └─> finance-service: cierra acreedor del empresario
```

La operación es **eventual**: si `commissions-service` está caído, el pago se
registra igualmente como “Fondos por liquidar” y el evento queda pendiente en el
outbox de commissions. Al recuperarse el servicio, el evento se publica y
finance cierra el pasivo. No se pierde el evento; el estado “Fondos por
liquidar” es transitorio.

## Verificación de cierre

- [x] `commissions-service` está en `docker-compose.yml` de producción.
- [x] Está en la lista de readiness de producción.
- [x] Gateway enruta sus rutas.
- [x] Prueba de readiness exige `commissions` en producción.
- [ ] Smoke en staging/producción: crear una venta directa y comprobar:
  - `commissions.comisiones` con la línea directa;
  - `venta.directa.acreditada` publicado;
  - `finance.ventas_lineas` con `directa=true`;
  - asiento `2.1.2` → `2.1.6`;
  - `GET /api/v2/empresarios/me/ventas` responde 200.

## Decisión de largo plazo

Se mantiene **`commissions-service` como única fuente de verdad económica**.
`finance-service` no recalcula comisiones ni clasifica ventas directas: consume
eventos. Duplicar esa lógica en finance crearía dos caminos para el mismo
asiento y riesgo de doble contabilización o divergencia.

La garantía de cierre es:

1. `commissions-service` desplegado y en `/ready`.
2. Outbox transaccional en la misma transacción que la comisión.
3. Reentrega de RabbitMQ + relay de outbox con `SKIP LOCKED`.
4. Idempotencia de finance por `event_id` y por `(order_id, linea_clave)`.
5. Alerta operativa si el outbox de commissions envejece o si una orden
   confirmada sigue sin subledger.

## Monitoreo y recuperación

Eventos pendientes en commissions:

```sql
SELECT tipo, COUNT(*) AS pendientes, MIN(creado_en) AS mas_antiguo
  FROM commissions.outbox
 WHERE estado = 'pendiente'
 GROUP BY tipo;
```

Alerta recomendada: `venta.directa.acreditada` o `comision.acreditada`
pendiente más de **5 minutos**, o `core_engine_outbox_pending_events` creciendo
sin publicar. La regla base ya existe en
`infra/prometheus/rules/alerts.yml` (`OutboxBacklog`).

Órdenes pagadas sin subledger en finance (posible evento perdido o commissions
caído):

```sql
SELECT p.order_id, MIN(p.creado_en) AS pagado_en
  FROM orders.pagos p
 WHERE p.estado = 'confirmado'
   AND NOT EXISTS (
         SELECT 1 FROM finance.ventas_lineas v WHERE v.order_id = p.order_id
   )
   AND p.creado_en < NOW() - INTERVAL '15 minutes'
 GROUP BY p.order_id;
```

Si aparece una orden aquí:

1. Verificar `/ready` y logs de `commissions-service`.
2. Revisar `commissions.outbox` y la DLQ de RabbitMQ.
3. Reiniciar el relay si está atascado; el evento se reentrega.
4. No crear el asiento a mano sin comprobar que el evento no llegó: se
   duplicaría al reintentar.

## Riesgo residual

La disponibilidad de `commissions-service` sigue siendo necesaria para cerrar el
acreedor. No hay un camino alternativo en `finance-service` que cierre la venta
directa desde `payment.procesado`; el diseño actual usa el outbox y la
reentrega. Si se quiere eliminar esa dependencia, el cambio sería:

- detectar líneas `comision=false` en `payment.procesado`;
- registrar la venta directa en `finance.ventas_lineas` con la misma clave
  idempotente `(order_id, linea_clave)`;
- dejar `venta.directa.acreditada` como evento redundante.

No se implementó porque duplicaría la lógica económica de commissions y el
outbox ya garantiza la entrega. Se reevalúa si se exige cierre síncrono o si
commissions deja de desplegarse.
