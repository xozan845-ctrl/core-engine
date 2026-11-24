# ADR-0043 — Cierre P0 para retail y flota/logística

- **Fecha:** 2026-11-20
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0039](./0039-rma-reembolso-nota-credito.md),
  [ADR-0042](./0042-exportacion-asincrona-purga-total-pdf-storage.md)

## Contexto

La auditoría de madurez vertical confirmó que el núcleo era sólido, pero el MVP
necesitaba cerrar correctness y canales de venta antes de profundizar verticales.
Se eligieron **retail** y **flota/logística** como focos complementarios.

## Decisión

1. **Analytics con devoluciones y cancelaciones.**
   - `devolucion.recibida` inserta hechos y comisiones negativas prorrateadas
     por línea, con `tipo='devolucion'`.
   - `devolucion.solicitada` de una cancelación total ajusta el saldo restante
     con `tipo='cancelacion'`, sin duplicar devoluciones parciales.
   - Las claves de ajuste son deterministas (`devolucion:<id>:<línea>` y
     `cancelacion:<orden>:<línea>`), por lo que reentregar el evento no duplica.
   - Los reportes siguen sumando `hechos_venta`/`comisiones`; al ser filas
     negativas, netean sin cambiar las consultas ni perder el original.
2. **Pago manual como modo explícito del MVP.**
   - El proveedor `manual` sigue siendo la vía soportada para efectivo,
     transferencia o conciliación externa: crea el pago pendiente y un admin lo
     confirma, rechaza, reembolsa o concilia.
   - La pasarela real (Stripe/BAC/POS) queda como adaptador de piloto; la
     arquitectura ya es configurable por tenant (`orders.configuracion_pagos`).
3. **Notificaciones multicanal y plantillas en BD.**
   - `identity.usuarios.telefono` y `PATCH /usuarios/{id}/telefono`.
   - `notifications.plantillas` permite overrides globales o por empresa para
     `email`, `whatsapp` y `sms`, con variables `{{pedido}}`, `{{total}}`,
     `{{estado}}`, `{{motivo}}`, `{{guia}}`, `{{cliente}}`, `{{vendedor}}`.
   - `NOTIFICACIONES_CANALES` elige canales; WhatsApp usa Meta Cloud API y SMS
     un HTTP genérico. Sin plantilla o canal, se conserva el correo actual.
   - API admin: `GET/PUT /api/v2/admin/notifications/plantillas`.
4. **Prueba de entrega (POD) para flota.**
   - `logistics.envios` guarda receptor, notas, foto, firma y timestamp.
   - `POST /api/v2/envios/{id}/prueba-entrega` registra la evidencia.
   - La capacidad `logistica_pod` (pack flota) exige POD antes de pasar a
     `entregada`; sin la capacidad, el flujo anterior sigue igual.
5. **Búsqueda por trigram.**
   - `pg_trgm` + índices GIN en `item.items` (nombre, código, código de barras) y
     `identity.partes` (nombre, nombre comercial, identificador).
   - No se introduce motor externo; full-text/Elastic queda fuera del MVP.
6. **Entrega de reportes por webhook.**
   - Si `destino` es `http(s)://`, analytics envía el contenido al terminar la
     ejecución y guarda `entrega_estado`, `entrega_error`, `entregado_en` e
     `intentos_entrega`.
7. **Documentación corregida.**
   - Agenda: la capacidad ya existía y permite clases/grupos.
   - Dental: la historia clínica y recetas ya están en la primitiva de registros.

## Consecuencias

- **Positivas:** los reportes de retail no sobreestiman ingresos tras RMA; la
  flota puede probar entregas; los clientes pueden recibir WhatsApp/SMS; el
  catálogo escala sin cambiar de motor.
- **Datos:** migraciones `99_analytics_devoluciones.sql`,
  `99_analytics_entrega_reportes.sql`, `99_notificaciones_plantillas.sql`,
  `99_usuario_telefono.sql`, `99_logistica_pod.sql` y `99_busqueda_trigram.sql`.
- **Contrato:** nuevas rutas de teléfono, plantillas, POD; campos de entrega en
  reportes.
- **Operación:** WhatsApp/SMS exigen credenciales y una plantilla aprobada por el
  proveedor cuando la conversación sale de la ventana de 24 h.
- **Límite:** el pago real sigue siendo un adaptador por piloto; el modo manual
  es el camino explícito del MVP.
- **Límite:** KDS, hotel, EHR profundo, transportistas externos y optimización
  de rutas quedan para la profundización vertical posterior.

## Criterios de aceptación

- Una devolución parcial reduce ventas y comisiones en analytics.
- Una cancelación total deja la orden en cero sin duplicar ajustes.
- Un envío de flota no pasa a `entregada` sin POD.
- Un admin guarda una plantilla y el siguiente evento usa ese texto.
- Un reporte con `destino` HTTP recibe el contenido y queda `entregado`.
- Las búsquedas de catálogo/clientes usan índices trigram.
