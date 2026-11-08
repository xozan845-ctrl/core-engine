# ADR-0001: Plataforma multi-vertical por capacidades

- **Fecha**: 2026-11-06
- **Estado**: Aceptado
- **Ámbito**: arquitectura, tenancy, contratos, catálogo, órdenes,
  fulfillment y scheduling

## Contexto

Core Engine ya tiene servicios para identidad, catálogo, tiendas, órdenes,
logística, finanzas, comisiones y operación de campo. El modelo actual mezcla
necesidades de distintos rubros y contiene un solapamiento entre
`logistics-service` y `field-service`.

La plataforma debe permitir que una empresa nueva elija una vertical y un pack,
active capacidades y opere sin cambios de código. Los tenants existentes deben
mantener su comportamiento, el aislamiento RLS no puede degradarse y los
contratos públicos deben evolucionar de forma aditiva.

No se busca construir un ERP universal, un microservicio por rubro ni un motor
de workflow genérico. Historia clínica y recetas clínicas no forman parte de
la primera fase.

## Decisión

### 1. Arquitectura por capacidades

Los bounded contexts se organizan por capacidad de negocio:

| Servicio | Responsabilidad |
|---|---|
| `identity-service` | tenants, empresas, miembros, roles, vertical, capacidades y configuración |
| `catalog-service` | ítems de tipo producto, servicio o alquiler; atributos e inventario |
| `stores-service` | tiendas, ofertas, listas de precios y canales |
| `orders-service` | compra, reserva, cotización y orden de servicio |
| `scheduling-service` | recursos, disponibilidad, citas, bloqueos y recordatorios |
| `logistics-service` | delivery, pickup, onsite y logística de flota |
| `finance-service` | ledger, facturación e impuestos |
| `commissions-service` | pagos, comisiones y liquidaciones |
| `shared` | `CapabilityGuard`, contratos comunes, validación, errores, Outbox y mensajería |

Se crea `scheduling-service` porque agenda, recursos, disponibilidad y
no-solapamiento son un bounded context compartido por belleza, dental y
field-service. La restricción de doble reserva se implementará con
`tstzrange` + índice GiST y será válida bajo concurrencia.

No se crean `dentista-service`, `farmacia-service` ni `pulperia-service`.

`field-service` queda como capacidad opcional de logística. Durante el
descubrimiento se decidirá si sus vehículos, rutas, paradas y tracking se
consolidan gradualmente en `logistics-service`; no será obligatorio para todos
los tenants.

### 2. Tenancy y capacidades

La evolución de identidad será aditiva:

- `identity.verticales(codigo, nombre, modelo_operacion)`;
- `identity.capacidades(codigo, nombre, modulo)`;
- `identity.vertical_capacidades` para defaults por vertical;
- `identity.empresa_capacidades(empresa_id, capacidad, config jsonb, activo)`;
- `identity.packs(codigo, version, vertical_codigo, config)` y
  `identity.pack_capacidades` para presets versionados;
- `identity.empresas.pack_codigo/pack_version` para conservar el pack elegido;
- `identity.empresas.vertical_id`;
- `tipo_empresa` se conserva como legado derivado;
- `modalidad_logistica` se conserva nullable/deprecado y se reemplaza
  gradualmente por capacidades y `fulfillment_config jsonb`.

La capacidad es la autoridad de negocio. El gateway autentica y enruta, pero no
decide reglas de negocio. Cada servicio valida `CapabilityGuard` antes de
ejecutar una operación. Una capacidad inactiva devuelve el error estructurado
`CAPACIDAD_NO_ACTIVA` y sus operaciones no se anuncian como disponibles en el
pack del tenant.

Las migraciones serán aditivas, reversibles y con backfill de tenants actuales.
No se elimina ni renombra una columna existente antes de la fase 6.

### 3. Catálogo y ofertas

Se introduce la abstracción `item` con `tipo` igual a `producto`, `servicio` o
`alquiler`.

- `track_inventory` controla si el ítem participa en stock;
- `duration_minutes` aplica a servicios;
- `attributes jsonb` se valida con JSON Schema por categoría;
- no se introduce EAV libre;
- lotes, vencimiento, unidades y proveedores se habilitan solo con la
  capacidad de inventario correspondiente;
- productos y servicios actuales se conservan durante la migración;
- `items_vista` será la superficie de convergencia antes de retirar el legado.

`stores.ofertas` usará `item_type + item_id`. El stock será nullable y los
servicios podrán declarar capacidad o slots.

### 3.1 Extensiones configurables por empresa

Para permitir nuevos rubros sin crear un servicio por negocio, cada empresa
podrá registrar extensiones versionadas sobre recursos estables de la
plataforma (`cliente`, `item`, `orden`, `cita`, `recurso`, entre otros). Una
extensión contiene:

- un JSON Schema para validar los datos;
- una definición de formulario declarativa para el cliente;
- reglas descriptivas en JSON, sin código ejecutable ni workflows genéricos;
- una versión activa por código y empresa.

Las referencias externas `$ref` y los documentos mayores a 64 KiB quedan
prohibidos. La validación se realiza en el servicio que registra la extensión
y la API expone además una operación explícita de validación. RLS mantiene el
alcance por empresa y los administradores pueden operar de forma global.

### 4. Órdenes y fulfillment

Las órdenes tendrán tipos explícitos: `compra`, `reserva`, `cotizacion` y
`orden_servicio`. Cada línea declarará su fulfillment: `delivery`, `pickup`,
`onsite` o `none`.

- una reserva crea una cita en `scheduling-service`;
- una compra no crea una cita;
- cada tipo de orden tendrá una máquina de estados explícita;
- no se implementará un workflow genérico.

Las mutaciones distribuidas seguirán RabbitMQ + Outbox, en línea con las reglas
vigentes de arquitectura. Las consultas síncronas entre servicios solo se
mantendrán cuando sean de lectura, idempotentes, estén documentadas y tengan
degradación o circuit breaker.

### 5. Pilotos y alcance aprobado

Los tres pilotos iniciales son farmacia/retail, belleza y flota:

| Capacidad | Retail | Farmacia | Belleza | Dental posterior | Flota |
|---|---:|---:|---:|---:|---:|
| Catálogo de ítems | Sí | Sí | Servicios | Servicios | Servicios |
| Inventario/stock | Sí | Sí | Opcional | Opcional | No |
| Lotes/vencimiento | No | Sí | No | No | No |
| Código de barras | Sí | Sí | Opcional | No | No |
| Agenda/recursos | No | No | Sí | Sí | Opcional |
| Historia clínica | No | No | No | Fuera de esta fase | No |
| Recetas | No | Fuera de fase 1 | No | Fuera de esta fase | No |
| Delivery/pickup | Opcional | Opcional | Opcional | No | Sí |
| Rutas/vehículos | No | No | No | No | Sí |
| Cotizaciones | Opcional | No | Opcional | Sí | Sí |
| Facturación | Sí | Sí | Sí | Sí | Sí |

El tenant dental se incorporará después de estabilizar `scheduling-service`,
con agenda y servicios únicamente. Historia clínica y recetas requerirán una
decisión posterior y, si entran en alcance, un `clinical-service` separado.

Farmacia sí incluye lotes, vencimiento y trazabilidad mínima de inventario.
La gestión completa de recetas queda fuera; puede existir una referencia
externa o un indicador `prescription_required` sin convertirlo en historia
clínica.

Flota incluye operación logística, servicios cotizables, órdenes de servicio,
facturación y liquidaciones. No incluye una tienda general de productos
físicos sin un caso comercial aprobado.

### 6. Glosario operativo

| Término | Definición |
|---|---|
| Tenant | Empresa aislada lógica y operativamente dentro de la plataforma |
| Empresa | Identidad comercial que posee tiendas, ítems, ofertas y órdenes |
| Vertical | Modelo de operación inicial del tenant, por ejemplo `retail` o `flota` |
| Capacidad | Funcionalidad activable por empresa, con configuración propia |
| Pack | Conjunto versionado de capacidades y configuración inicial de una vertical |
| Ítem | Unidad vendible de catálogo: producto, servicio o alquiler |
| Oferta | Publicación comercial de un ítem en una tienda o canal |
| Fulfillment | Forma de cumplir una línea: delivery, pickup, onsite o none |
| Recurso | Profesional, sala, sillón, vehículo u otro recurso reservable |
| Cita | Reserva temporal de uno o más recursos en `scheduling-service` |
| Outbox | Registro transaccional local de eventos pendientes de publicación |
| RLS | Row Level Security que impide que un tenant lea o modifique otro tenant |

### 7. Inventario inicial de acoplamientos

Este es el inventario base observado en el repositorio; se convertirá en el
plan detallado de migración durante la fase 0:

| Área | Acoplamiento actual | Tratamiento |
|---|---|---|
| Identidad | `identity.empresas`, miembros, roles, configuración y contexto de tenant | Convertir en autoridad de verticales y capacidades; preservar IDs y RLS |
| Catálogo | `catalog.productos`, `catalog.servicios`, stock, reservas y eventos | Migración aditiva hacia ítems; mantener vistas y consumidores existentes |
| Tiendas | `stores.ofertas`, tiendas, precios y consumidor de `stock.updated` | Añadir `item_type + item_id`, slots y guardas de capacidad |
| Órdenes | carrito, proyección de órdenes, estados y Outbox | Añadir tipos y fulfillment por línea; conservar contratos actuales |
| Logística | envíos y asignaciones consumen eventos de órdenes/pagos | Extender a delivery, pickup y onsite |
| Field service | `field.vehiculos`, rutas, paradas, visitas y tracking | Mantener inicialmente; evaluar consolidación con logística |
| Finanzas | ledger, comprobantes, impuestos y consultas a órdenes/identidad | Consumir eventos y aplicar capacidades sin duplicar facturación |
| Comisiones | comisiones, pagos, liquidaciones y consultas a órdenes | Mantener como capacidad transversal de liquidación |
| Gateway | proxy a todos los servicios actuales | Mantener routing/auth; no trasladar decisiones de capacidad aquí |
| Shared | RabbitMQ, Outbox, DLQ, errores y métricas | Añadir `CapabilityGuard`, contratos de atributos y errores comunes |

Los acoplamientos directos observados entre servicios —por ejemplo, consultas de
órdenes a catálogo, logística a órdenes/identidad y finanzas/comisiones a
órdenes— se revisarán por operación. Las mutaciones críticas no se harán
dependientes de HTTP síncrono.

## Roadmap aprobado

| Fase | Entregable | Criterio de aceptación |
|---|---|---|
| 0 | ADR, glosario, matriz e inventario de acoplamientos | ADR aprobado, tres pilotos definidos y plan de migración |
| 1 | Tenancy, capacidades, flags, `CapabilityGuard` y migraciones | Tenant dental/salón/flota creado sin logística obligatoria; RLS verde |
| 2 | Ítems, atributos, ofertas de servicios e inventario opcional | Belleza vende un servicio y retail continúa operando |
| 3 | Tipos de orden, fulfillment y cotizaciones | Compra, pickup y onsite funcionan con contratos verdes |
| 4 | `scheduling-service` | Doble reserva imposible bajo concurrencia; reserva crea cita |
| 5 | Packs versionados | Cada piloto se crea desde un pack sin cambios de código |
| 6 | Migración progresiva y deprecación | Pilotos operando, runbooks actualizados y sin incidentes críticos |

Cada fase se implementará en una rama/PR separado: `feat/tenancy-capacidades`,
`feat/catalog-items`, `feat/orders-types`, `feat/scheduling` y
`feat/vertical-packs`.

## Consecuencias

### Positivas

- Un nuevo rubro compone capacidades existentes en lugar de duplicar servicios.
- Los tenants actuales conservan sus tablas, contratos y comportamiento durante
  la migración.
- `scheduling-service` concentra reglas de disponibilidad y no-solapamiento
  que realmente son compartidas.
- Las capacidades pueden apagarse con flags y migraciones aditivas como
  mecanismo de rollback.

### Costes y riesgos

- Durante la transición coexistirán productos/servicios con la abstracción
  `item`.
- Habrá que mantener compatibilidad entre `field-service` y
  `logistics-service` hasta resolver la consolidación.
- Los packs requieren versionado, validación de configuración y pruebas de
  contrato para no convertir defaults en lógica oculta.
- La agenda necesita pruebas de concurrencia, índices GiST y una estrategia
  clara de idempotencia para eventos.

## Criterios de aceptación globales

- Un tenant nuevo de retail/farmacia, belleza y flota se crea y opera sin tocar
  código.
- Los tenants existentes no cambian su comportamiento.
- Una capacidad deshabilitada produce `CAPACIDAD_NO_ACTIVA` y no expone sus
  operaciones como disponibles.
- No existen servicios duplicados por rubro.
- Las tablas nuevas tienen RLS, políticas y pruebas de aislamiento.
- Los contratos nuevos son opcionales y no rompen consumidores existentes.
- p95 permanece dentro de la línea base y no aparecen hallazgos altos o
  críticos nuevos.
- ADR, matriz, runbooks y OpenAPI se actualizan en la misma fase que el cambio.
