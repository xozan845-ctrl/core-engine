# ADR-0040 — PDF completo y almacenamiento de comprobantes

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0031](./0031-facturacion-electronica-pdf.md),
  [ADR-0038](./0038-iva-por-linea-checkout.md)

## Contexto

El PDF existente solo mostraba serie, número y totales. No incluía emisor,
cliente, líneas ni desglose de IVA, y se regeneraba en cada descarga sin
persistencia.

## Decisión

1. **Contenido completo.** El PDF incluye:
   - emisor y RUC emisor;
   - cliente, RUC y orden/devolución;
   - líneas con SKU, cantidad, base, IVA y total;
   - totales, estado y referencia electrónica.
2. **Paginación.** El contenido se reparte en páginas de 42 líneas y el PDF
   declara `/Kids`/`/Count` correctamente.
3. **Datos de línea.** Al emitir una factura se guardan los ítems en
   `datos_cliente.items` con base/IVA/total. Las notas de crédito ya los traen
   desde el RMA.
4. **Almacenamiento.** La primera descarga genera el PDF y lo persiste en
   `finance.comprobante_pdf` (`bytea`). Las siguientes descargas devuelven la
   copia almacenada.
5. **Sin dependencias externas.** Se mantiene el generador PDF 1.4 con
   Helvetica, suficiente para representación impresa y auditoría.

## Consecuencias

- **Positivas:** el PDF es auditable y consistente con el comprobante; no se
  regenera en cada descarga; soporta pedidos largos.
- **Datos:** migración `99_comprobante_pdf.sql`; RLS solo para el rol de
  servicio.
- **Object storage:** ADR-0042 mueve el PDF a `storage-service` y deja la base
  como fallback.
- **Límite:** el XML/representación gráfica oficial DGI sigue fuera del núcleo.

## Criterios de aceptación

- El PDF contiene RUC emisor, cliente, SKU y desglose de IVA.
- El total del PDF coincide con el comprobante.
- La segunda descarga devuelve los mismos bytes almacenados.
- Un pedido con muchas líneas genera más de una página.
