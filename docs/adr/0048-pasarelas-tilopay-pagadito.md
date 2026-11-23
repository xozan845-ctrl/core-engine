# ADR-0048 — Pasarelas concretas: Tilopay primaria y Pagadito secundaria

- **Fecha:** 2026-11-23
- **Estado:** Aceptado (implementado; verificación sandbox pendiente de credenciales)
- **Relacionado:** [ADR-0027](./0027-pagos-proveedor-reembolsos.md),
  [ADR-0037](./0037-proveedores-pago-configurables.md)

## Contexto

La ACL `ProveedorPagos` y el catálogo `orders.proveedores_pago` existían, pero
solo había adaptadores `simulado`, `http` genérico y `manual`. Para operar en
Nicaragua hacía falta una pasarela real con checkout alojado, reembolsos,
consulta de estado y conciliación sin acoplar el dominio a un proveedor.

La comparación de Tilopay y Pagadito concluyó:

- **Tilopay** expone REST/JSON v1, checkout alojado, `consult`,
  `processModification` para reembolsos totales/parciales y liquidaciones con
  comisiones e impuestos. En Nicaragua opera con BAC y su tarifa es la del banco
  más 1 %.
- **Pagadito** está muy establecida en Nicaragua, soporta NIO y tiene webhooks
  firmados asimétricamente con reintentos, pero su API es APIPG (formularios
  con claves de operación públicas y respuestas JSON), no documenta reembolsos
  por API y su conciliación es más manual.

## Decisión

1. **Dos adaptadores concretos detrás de la misma ACL**, seleccionados por
   configuración (`orders.proveedores_pago.tipo`), nunca por código de dominio:
   - `tilopay` → `src/pasarelas/tilopay.rs`
   - `pagadito` → `src/pasarelas/pagadito.rs`
   - El catálogo se siembra inactivo en `99z_pasarelas.sql`; activarlo es una
     decisión operativa por tenant o global.
2. **Tilopay es la pasarela primaria** (mejor API, reembolsos y liquidaciones).
   **Pagadito queda como alternativa por tenant** para comercios que lo
   prefieran o ya lo tengan afiliado.
3. **Cero secretos y cero URLs en código.** `config.base_url` y los nombres de
   variables (`apiuser_env`, `password_env`, `key_env`, `uid_env`, `wsk_env`)
   salen de la configuración del proveedor. Las credenciales se leen del
   entorno. El tenant no puede sobrescribir URLs ni nombres de variable.
4. **Cargo externo ≠ orden pagada.** Un método con `cargo_externo=true` crea el
   link/transacción y deja el pago `pendiente`, salvo el proveedor `simulado`.
   Solo la confirmación de la pasarela (consulta o conciliación) mueve la orden
   a `pagada`. Un método distinto de `simulado` no puede usar el proveedor
   simulado sin `config.simulado=true`: así una activación accidental no cobra
   gratis. Un método distinto de `simulado` no puede usar el proveedor
   simulado sin `config.simulado=true`: así una activación accidental no cobra
   gratis.
5. **El webhook es una señal, no una fuente de verdad.** Los endpoints
   públicos `POST /api/v2/pagos/tilopay/webhook` y
   `POST /api/v2/pagos/pagadito/webhook` extraen la referencia
   (`order_id`/`orderNumber` en Tilopay; `resource.ern` en Pagadito), buscan el
   pago y **consultan la pasarela** antes de confirmar o rechazar. La firma
   asimétrica de Pagadito y el `orderHash` de Tilopay son defensa en
   profundidad pendiente de credenciales/especificación; no son la barrera de
   seguridad.
6. **Referencia de la transacción.** `orders.pagos.referencia_proveedor` guarda
   el `orderNumber` de Tilopay o el `token` de Pagadito. `referencia` sigue
   siendo la referencia propia (order id) para localizar el pago. Reembolsos y
   consultas usan la referencia del proveedor cuando existe.
7. **Conciliación programada.** Cada
   `PAGOS_CONCILIACION_INTERVALO_SEGUNDOS` (900 s por defecto) se consultan
   hasta 200 pagos `pendiente` con proveedor externo creados hace más de
   `PAGOS_CONCILIACION_MINUTOS` (10 min). Cubre webhooks perdidos. Comparte con
   el webhook `FlujoPagos` para no duplicar transiciones.
8. **Carrera de expiración.** Si la pasarela aprueba un pago cuando la orden ya
   está `cancelada`/`devuelta`/`rechazada`, no se resucita la orden: se registra
   `pago.tardio` en `orders.pagos_eventos` y se deja el pago confirmado para
   reembolso manual. Un rechazo sobre una orden ya avanzada registra
   `pago.rechazado_tardio`. Al cancelar una orden, los pagos manuales
   pendientes se rechazan, pero los cargos de pasarela externa se dejan
   pendientes para que la conciliación detecte un cobro posterior (ventana de
   30 días).
9. **Arranque seguro.** `PAGO_PROVEEDOR` desconocido o una pasarela sin
   credenciales/URL impide arrancar; no se degrada silenciosamente a simulado.

## Consecuencias

- **Positivas:** coexistencia de pasarelas por tenant sin bifurcar el dominio;
  reembolsos parciales reales con Tilopay; conciliación automática; los
  secretos siguen en el entorno; el checkout devuelve `pago_url` para el
  frontend.
- **Contrato:** `OrderView.pago_url` (opcional) y
  `Pago.referencia_proveedor` (aditivo). Nuevas rutas públicas de webhook
  registradas en gateway y contrato.
- **Datos:** `99z_pasarelas.sql` añade `referencia_proveedor`, siembra los
  proveedores inactivos y el método `tarjeta` (también inactivo hasta que el
  admin configure y active una pasarela).
- **Limitaciones conocidas:** Pagadito no reembolsa ni contracarga por API (se
  hace desde su panel); la firma de webhooks de Pagadito y el `orderHash` de
  Tilopay no se validan aún localmente; la verificación con sandbox real queda
  pendiente de credenciales.
- **Impuestos de pasarela:** las comisiones/IVA/retenciones que devuelve
  Tilopay en `consult` todavía no se persisten; es el siguiente paso para
  conciliación contable fina.

## Criterios de aceptación

- Con `PAGO_PROVEEDOR=simulado` nada cambia y no se exigen credenciales.
- Con `PAGO_PROVEEDOR=tilopay|pagadito` sin credenciales el servicio no arranca.
- Un método `tarjeta` con pasarela real deja el pago `pendiente` hasta que la
  pasarela lo confirme.
- Los tests de `pasarelas::` cubren login/creación/consulta/reembolso de
  Tilopay y connect/exec_trans/get_status de Pagadito contra un servidor HTTP
  simulado, incluidos los códigos de error.
- Un webhook repetido es idempotente y no vuelve a transicionar la orden.
- Un pago aprobado para una orden expirada deja `pago.tardio` y no entrega.
- La conciliación no confirma pagos que la pasarela reporta como pendientes.
