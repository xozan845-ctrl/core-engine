# ADR-0030 — Reseñas moderadas y wishlist por comprador

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0011](./0011-stores-service-en-rust.md),
  [ADR-0028](./0028-cupones.md)

## Contexto

El storefront no tenía señales de comprador: no había reseñas ni lista de
deseos. El diagnóstico de agnosticismo lo marcaba como deuda de retail.

## Decisión

1. **Reseñas en stores.** `stores.resenas` guarda una reseña por comprador e
   item, con calificación 1–5, título, comentario y estado
   `pendiente|publicada|rechazada`.
2. **Moderación explícita.** La reseña nace `pendiente`; solo un admin la
   publica. El listado público solo devuelve `publicada` y calcula promedio.
3. **Autoría del comprador.** Un comprador autenticado crea/edita/elimina su
   reseña; editar la vuelve a `pendiente`.
4. **Wishlist en stores.** `stores.favoritos` guarda `(comprador, item,
   empresa)`; agregar es idempotente y eliminar es por item.
5. **Sin compra verificada en esta fase.** No se consulta orders para validar
   la compra; queda como evolución (requiere endpoint interno y política de
   producto).

## Consecuencias

- **Positivas:** el storefront gana contenido social y lista de deseos sin
  acoplar un servicio nuevo; la moderación evita publicar spam.
- **Contrato:** rutas públicas de reseñas, rutas de comprador y moderación
  admin; wishlist bajo `/api/v2/comprador/favoritos`.
- **Datos:** migración `99_resenas_wishlist.sql`; RLS de lectura pública,
  comprador y admin.
- **Pendiente:** compra verificada, respuestas del vendedor, imágenes en
  reseñas y contador de favoritos en el catálogo.

## Criterios de aceptación

- Una reseña nueva no aparece en el listado público hasta publicarse.
- Un comprador no puede moderar reseñas (`403`).
- El promedio público refleja solo reseñas publicadas.
- Agregar dos veces el mismo favorito no duplica.
- La eliminación de favorito es idempotente.
