# ADRs de la plataforma multi-vertical

Este directorio contiene las decisiones de arquitectura que guían la
convergencia de Core Engine hacia una plataforma multi-vertical. El registro
histórico del repositorio permanece en [`../decisiones.md`](../decisiones.md);
este índice evita mezclar las decisiones nuevas con las ADR históricas.

## Índice

| ADR | Estado | Decisión |
|---|---|---|
| [0001](./0001-plataforma-multi-vertical.md) | Aceptado | Capacidades compartidas por tenant, tres pilotos iniciales y `scheduling-service` como nuevo bounded context |
| [0002](./0002-envelope-y-desacoplamiento.md) | Aceptado (fase 3 casi completa; falta `identity`) | Envelope `{data,meta}` por servicio y desacoplamiento de `orders`/`scheduling`/`field` mediante APIs internas |
| [0003](./0003-notificaciones-correo.md) | Aceptado (fases 1-2; notificaciones pendientes) | Proveedor de correo intercambiable, notificaciones del pedido y reset de contraseña |

## Convención

- Los archivos usan `NNNN-kebab-case.md`.
- Cada ADR debe indicar fecha, estado, contexto, decisión, consecuencias y
  criterios de aceptación.
- Los cambios estructurales posteriores deben enlazar este ADR y registrar una
  nueva decisión o una enmienda explícita; no se modifica silenciosamente el
  alcance aceptado.
