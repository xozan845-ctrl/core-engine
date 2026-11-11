# ADR-0002: Envelope de respuestas y desacoplamiento entre servicios

- **Fecha**: 2026-11-10
- **Estado**: Aceptado — fase 1 implementada en `commissions-service`
- **Ámbito**: contrato público `/api/v2`, paginación, límites entre bounded contexts

## Contexto

Dos deudas siguen abiertas en la auditoría y no son fallos de gate:

1. **G-5 / R-C-1**: las respuestas de `/api/v2` no usan el envelope
   `{ data, meta }`. Los servicios devuelven DTOs o `Pagina<T>` directamente y
   el envelope no puede introducirse en `/api/v1` sin una migración versionada.
   `/api/v2` ya existe, así que la ventana de migración es ahora.
2. **Acoplamiento entre esquemas**: `orders` lee `identity.empresa_capacidades`,
   `scheduling` y `field` leen `identity.partes`, e `identity` escribe
   `stores.tiendas`. Son lecturas/escrituras directas a tablas de otro
   bounded context.

La implementación inmediata de ambos puntos toca `stores`, `field` e
`identity`, que tienen trabajo activo en otros worktrees. Este ADR fija la
decisión y el orden de migración para ejecutarla en PRs atómicos sin mezclar
frentes.

## Decisión

### 1. Envelope en la capa de servicio, no en el gateway

Se implementará un interceptor compartido en `@core/shared`:

```ts
interface RespuestaApi<T> {
  data: T;
  meta: {
    timestamp: string; // ISO 8601 UTC con Z
    path: string;
    method: string;
    statusCode: number;
  };
}
```

- Se aplica a los controladores públicos `/api/v2` de cada servicio.
- **No** se aplica a `/internal/*` ni a `/health`, `/metrics`, `/docs` o
  respuestas binarias de storage.
- No se envuelven los errores: siguen con `{ codigo, mensaje, detalles? }`
  (R-C-8).
- El gateway no reescribe cuerpos: solo enruta y propaga el envelope. Esto evita
  bufferizar streams, romper descargas de imágenes/video o fingir un contrato
  que el servicio no produce.

La paginación viaja dentro de `data`:

```json
{
  "data": {
    "items": [],
    "total": 0,
    "pagina": 1,
    "limite": 20,
    "paginas": 1
  },
  "meta": { "timestamp": "...", "path": "...", "method": "GET", "statusCode": 200 }
}
```

> **Decisión de la fase 1:** se conservan los nombres actuales de `Pagina<T>`
> (`items/total/pagina/limite/paginas`) dentro de `data`, para no cambiar dos
> veces el contrato y mantener consistencia con el dominio. R-C-1 queda
> enmendada con ese shape.

### 2. Desacoplamiento por APIs internas

- `identity` es la única autoridad de `empresas`, `partes`, `capacidades`,
  `packs` y `personal`.
- `stores` es la única autoridad de `tiendas`, `ofertas` y asignaciones
  vendedor–tienda.
- `orders`, `scheduling` y `field` dejan de leer tablas de `identity`; consumen
  sus endpoints internos existentes (`/internal/...`) mediante
  `service-clients.ts`, con timeout y degradación controlada.
- `identity` deja de escribir `stores.tiendas`; la asignación se hace por la API
  interna de `stores`.
- Las consultas síncronas solo se aceptan para lecturas idempotentes y
  documentadas (R-AR-2); las mutaciones críticas siguen por eventos/Outbox.

## Plan de migración

Cada fase es un PR atómico y no mezcla envelope con desacoplamiento.

| Fase | Alcance | Criterio de salida |
|---|---|---|
| 0 | Enmienda de R-C-1 con los nombres de paginación elegidos y contrato `RespuestaApi<T>` | Regla y ADR sin contradicción |
| 1 | Interceptor compartido + tests puros + un servicio piloto (`commissions`) | ✅ `commissions` devuelve envelope; test de contrato y OpenAPI actualizados |
| 2 | `finance`, `orders`, `logistics` | ⏳ `finance` completado; faltan `orders` y `logistics` |
| 3 | `catalog`, `stores`, `scheduling`, `identity`, `field`, `storage`, `market-intelligence` | Todos los servicios públicos con envelope |
| 4 | Gateway, smoke, QA harness y OpenAPI agregado | `npm run demo` verde y contrato público sin respuestas sin envolver |
| 5 | Desacoplamiento de `orders`/`scheduling`/`field` hacia APIs internas | Sin SQL directo a tablas de identity; pruebas de caída con 503/degradación |
| 6 | Mover la asignación vendedor–tienda de identity a stores | Identity sin escrituras a `stores.tiendas`; RLS y tests verdes |

## Consecuencias

### Positivas

- Un contrato único y testeable para clientes y OpenAPI.
- Los servicios dejan de depender de tablas ajenas y pueden evolucionar su
  esquema sin romper a los vecinos.
- La migración es incremental y cada fase puede revertirse apagando el
  interceptor o volviendo a la lectura anterior.

### Costes y riesgos

- El envelope rompe consumidores actuales de `/api/v2`; hay que actualizar
  smoke, QA harness, tests de contrato y OpenAPI en la misma fase.
- La lectura de tablas ajenas es hoy una dependencia funcional; sustituirla por
  HTTP exige timeouts, degradación y pruebas de caída.
- Las fases 3 y 6 tocan `stores`, `field` e `identity`; no deben ejecutarse
  mientras esos worktrees tengan cambios sin integrar.

## Criterios de aceptación

- Ninguna respuesta pública `/api/v2` sale sin `{ data, meta }`; los errores
  conservan `{ codigo, mensaje, detalles? }`.
- La paginación tiene un único shape, documentado y probado.
- `grep` de tablas `identity.*` en `orders`, `scheduling` y `field` no encuentra
  lecturas directas fuera de migraciones o pruebas.
- `identity` no escribe en `stores.tiendas`.
- `npm run demo`, integración y OpenAPI quedan verdes en cada fase.
