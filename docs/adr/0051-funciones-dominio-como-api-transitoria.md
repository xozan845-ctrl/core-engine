# ADR-0051 — Funciones de dominio como API transitoria entre esquemas

- **Fecha:** 2026-11-25
- **Estado:** Aceptado (implementado; transitorio)
- **Relacionado:** R-DB-1, R-MS-16, ADR-0050, auditoría `2026-11-24-auditoria-contrato-servicio`

## Contexto

La auditoría de contrato de servicio encontró cuatro **escrituras directas** de
un servicio al esquema de otro:

1. `inventory` → `item.item_series` (marcar vendidas/liberadas/devueltas).
2. `pricing` → `item.items.tipo_precio`.
3. `identity` → `inventory.ubicaciones` (ubicación principal del tenant).
4. `identity` → `finance.configuracion_fiscal` (cambio de región fiscal).

Las cuatro ocurren dentro de una transacción local que también actualiza el
estado del servicio llamador. Moverlas a HTTP o a eventos en esta fase
introduciría ventanas de inconsistencia (stock reservado sin serie, empresa sin
ubicación, región cambiada sin configuración fiscal) o exigiría una saga nueva.
A la vez, el DML directo viola la propiedad de datos (R-DB-1/R-MS-16) y acopla
los esquemas.

## Decisión

Sustituir el DML directo por **funciones de dominio en el esquema dueño**,
invocadas desde la misma transacción del llamador:

| Función | Dueño | Llama |
|---|---|---|
| `item.marcar_series_vendidas(uuid, uuid[], int)` | `item` | `inventory` |
| `item.liberar_series(uuid, uuid[], uuid)` | `item` | `inventory` |
| `item.devolver_series(uuid, uuid[], uuid)` | `item` | `inventory` |
| `item.marcar_tipo_precio(uuid, uuid, text)` | `item` | `pricing` |
| `inventory.crear_ubicacion_predeterminada(uuid, uuid)` | `inventory` | `identity` |
| `finance.actualizar_region_fiscal(uuid, text)` | `finance` | `identity` |

Propiedades:

- La función vive en `infra/db/init/99_domain_functions.sql`, en el esquema del
  servicio dueño; su contrato se revisa en el PR de ese servicio.
- Se ejecuta con los permisos del rol de aplicación y en la **misma
  transacción** del llamador, preservando atomicidad.
- El servicio llamador no puede hacer `INSERT/UPDATE/DELETE` sobre tablas ajenas;
  sólo `SELECT <esquema>.funcion(...)`.
- La suite `services-contract` añade el check duro `cross-schema-write`: **cero**
  DML directo a esquemas ajenos. Las lecturas cross-schema siguen siendo deuda
  explícita (P2) hasta sustituirlas por proyecciones.
- Los esquemas de prueba de `inventory` y `pricing` replican las funciones para
  que las suites PostgreSQL sigan siendo deterministas.

## Alternativas descartadas (por ahora)

| Alternativa | Por qué no en esta fase |
|---|---|
| HTTP interno síncrono | Rompe la atomicidad de stock/serie y exige compensación; además R-AR-2 prohíbe mutaciones críticas por HTTP. |
| Evento + consumidor en `item` | Requiere mover la reserva de series a un modelo local con bloqueo y resolver la carrera de doble venta; es la Fase 2. |
| Mover la tabla al esquema del llamador | Cambia la propiedad del dato y obliga a migrar catálogo, reportes y RLS. |
| Mantener DML directo | Viola R-DB-1/R-MS-16 y deja el acoplamiento invisible. |

## Consecuencias

**Positivas**

- Cero escrituras directas a tablas ajenas; la propiedad del dato queda
  explícita en el esquema dueño.
- Atomicidad preservada sin nuevas sagas.
- El check `cross-schema-write` impide reincidir.

**Negativas / riesgos**

- Sigue existiendo acoplamiento a nivel de base de datos (mismo PostgreSQL). No
  es una frontera de servicio completa.
- Cambiar la firma de una función exige coordinar al llamador y al dueño.
- Las funciones deben mantenerse y probarse en los esquemas de prueba.

**Neutras**

- Las lecturas cross-schema (`item` desde `inventory`, `identity` desde
  `finance`, etc.) no se resuelven aquí; se congelan en el baseline y se
  priorizan en la Fase 2.

## Criterios de aceptación

- [x] Las 4 escrituras directas desaparecen del código de los servicios.
- [x] `services-contract` reporta `cross-schema-write` vacío.
- [x] `inventory`, `pricing`, `identity` y `finance` compilan y pasan sus tests.
- [x] Las funciones están versionadas y con permisos condicionados al rol.
- [x] La deuda de arquitectura baja de 34 a 29 entradas (4 readiness + 1
  acoplamiento eliminado).

## Seguimiento

1. Mover `inventory`/`pricing` a eventos cuando exista un modelo local de
   reserva de series con bloqueo (Fase 2).
2. Sustituir lecturas cross-schema por proyecciones locales.
3. Evaluar si las funciones deben pasar a una API HTTP interna cuando el
   servicio dueño tenga un ciclo de despliegue independiente real.
