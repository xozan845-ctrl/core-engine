# ADR-0021: negocio como unidad operativa dentro de la empresa

- **Fecha**: 2026-11-15
- **Estado**: Aceptado (fases 1-4, reportes, liquidaciones, selector, permisos, RLS, inventario, facturación, auditoría, administración, analítica, cohortes y reportes programados en `analytics-service`)
- **Ámbito**: identidad, tenancy, roles, personal, invitaciones, tiendas
- **Relacionado**: [ADR-0001](./0001-plataforma-multi-vertical.md),
  [ADR-0017](./0017-identity-service-en-rust.md),
  [ADR-0049](./0049-regiones-tributarias.md)

## Contexto

El modelo vigente trata a la empresa como un tenant con **una sola vertical y
un solo pack**:

- `identity.empresas.vertical_id` es una columna única
  (`infra/db/init/24_multi_vertical.sql`);
- el pack pertenece a una vertical y el alta rechaza combinaciones cruzadas
  (`infra/db/init/29_vertical_packs.sql`,
  `packages/identity-service/src/dominio/empresas.rs`);
- los roles se crean por empresa (`identity.roles_empresa`);
- las invitaciones se emiten por empresa (`identity.invitaciones_personal`);
- el usuario guarda un solo `tenant_id` y un solo rol global
  (`identity.usuarios`).

Eso obliga a que una empresa que opera retail y farmacia, o servicios y
logística, se divida en **varias empresas**. El coste es alto: se duplica el
perfil fiscal, la facturación, los roles y la experiencia del dueño, y no existe
un selector de tenant para que una misma persona administre varias empresas.

La regla de negocio acordada es:

1. Una empresa simple funciona sin pasos adicionales.
2. Una empresa puede tener **varios negocios de distinto tipo**.
3. Cada negocio define sus propios roles, puestos, personal e invitaciones.
4. Añadir un segundo negocio exige un **empresario activo** de la empresa.

## Decisión

La empresa sigue siendo el **tenant fiscal y el límite de RLS**. Se introduce
`identity.negocios` como **unidad operativa** que aporta vertical, pack y
alcance de personal.

```text
empresa (RUC, facturación, RLS, capacidades)
  └── negocio (vertical_id + pack + estado)
        ├── roles_empresa.negocio_id (opcional; NULL = transversal)
        ├── puestos_empresa.negocio_id
        ├── asignaciones_personal.negocio_id
        ├── invitaciones_personal.negocio_id
        └── stores.tiendas.negocio_id
```

### Reglas

| Regla | Implementación |
|---|---|
| Empresa simple sin pasos extra | Al crear la empresa se inserta el negocio `principal` en la misma transacción |
| Varios tipos de negocio | `negocios.vertical_id` + `negocios.pack_codigo`, con unicidad por `(empresa_id, codigo)` |
| Cada negocio con sus roles | `roles_empresa.negocio_id` opcional; `NULL` = rol transversal, con `UNIQUE (negocio_id, codigo)` por negocio |
| Cada negocio con su personal | `asignaciones_personal.negocio_id`; las invitaciones se acotan al negocio |
| Segundo negocio exige empresario | `Negocios::crear` exige una asignación activa con rol `empresario` **transversal** (`negocio_id IS NULL`) cuando ya existe al menos un negocio |
| Compatibilidad | Backfill de un negocio `principal` por empresa; roles y puestos existentes quedan transversales (`negocio_id NULL`) |
| Aislamiento | RLS se mantiene por `empresa_id`; el negocio nunca cruza empresas |

### Capacidades

`identity.negocio_capacidades` es la autoridad por negocio. Reglas:

- El negocio principal hereda en el backfill el conjunto que ya tenía la
  empresa, incluidos add-ons fuera del pack.
- Los negocios adicionales arrancan con las capacidades de su pack.
- El endpoint interno `POST /internal/empresas/capacidades` acepta
  `negocio_id`; el guard de cada servicio lo envía desde `x-negocio`.
- Si el negocio no tiene configuración propia (tenant legacy), el guard cae a
  `empresa_capacidades`.
- Un cambio de capacidad a nivel de empresa se replica en todos sus negocios.
- Un cambio a nivel de negocio actualiza el resumen de empresa como unión
  (`bool_or`) para no romper clientes legados.

La API pública añade:

- `GET /api/v2/empresas/{empresaId}/negocios/{negocioId}/capacidades`
- `PATCH /api/v2/empresas/{empresaId}/negocios/{negocioId}/capacidades/{capacidad}`

La autorización usa rol `empresario` (transversal o del negocio) o el permiso
`capacidades.leer` / `capacidades.gestionar` en `roles_empresa.permisos`.

### API

- `GET /api/v2/empresas/{empresaId}/negocios`
- `POST /api/v2/empresas/{empresaId}/negocios`
- `negocio_id` opcional en roles, puestos, asignaciones e invitaciones.
- Si se omite en puestos, asignaciones o invitaciones, se usa el negocio
  principal. En roles, omitirlo crea un rol transversal a la empresa.

### Autorización por negocio y permisos

El JWT incluye `negocio_id` y el gateway lo propaga como `x-negocio`. La
autorización fina vive en `identity-service`:

- `super_admin`/`admin` conservan acceso global.
- Un rol `empresario` transversal (`negocio_id NULL`) administra toda la
  empresa.
- Un rol `empresario` acotado a un negocio administra solo ese negocio.
- Cualquier rol puede administrar si su `permisos.acciones` declara:
  - `personal.leer` para listar roles, puestos, asignaciones e invitaciones;
  - `personal.gestionar` para crearlos, modificarlos o revocarlos.
- Las listas se filtran al negocio del actor salvo que tenga alcance global.
- El gateway deja de decidir roles para estas rutas y delega en el servicio,
  que es la autoridad de negocio (ADR-0001).

### Tiendas y órdenes

- `stores.tiendas.negocio_id` se expone en el contrato y se asigna al crear la
  tienda; si el actor no trae negocio se usa el principal.
- Las ofertas internas incluyen `negocio_id` para que órdenes pueda propagarlo.
- `ItemOrden.negocio_id` viaja en el evento y en `items_json`; la proyección
  `orders.orden_vista.negocio_id` se rellena con un trigger cuando todas las
  líneas comparten negocio.
- `GET /api/v2/orders/empresa` acepta `negocioId` y filtra; los actores con
  negocio en sesión quedan acotados automáticamente.
- El checkout valida capacidades por par `(empresa, negocio)`, no por empresa
  agregada.
- Las listas de tiendas y pedidos de un actor con `x-negocio` se recortan a ese
  negocio.

### Catálogo e inventario

- `catalog.items.negocio_id` con backfill al negocio principal y trigger de
  default para altas sin contexto.
- El alta de items y alquileres toma el negocio del actor o del cuerpo, y la
  lista filtra por negocio.
- La lectura/edición de un item comprueba que el negocio coincida con el del
  actor.
- Los niveles de stock y los movimientos de inventario filtran por el negocio
  del item (`catalog.items.negocio_id`), sin duplicar la columna en cada tabla
  de inventario.

### Logística y reportes

- `logistics.envios.negocio_id` y unicidad por `(orden, empresa, negocio)`.
- La guía se agrupa por empresa y negocio; la flota se deriva con
  `activas_en(empresa, negocio)`.
- `GET /api/v2/orders/empresa/resumen` agrega ventas por negocio con filtros
  `negocioId`, `desde` y `hasta`.
- El actor con negocio en sesión queda acotado automáticamente.

### Finanzas y comisiones

- `analytics.comisiones.negocio_id` se propaga desde la línea de la orden.
- `GET /api/v2/empresarios/me/reportes/negocios` agrega ventas, comisiones y
  monto del empresario por negocio, con `negocioId`, `desde` y `hasta`.
- `finance.comprobantes.negocio_id` se hereda de la orden al emitir.
- `GET /api/v2/finanzas/libro-ventas/negocios` agrega base, IVA, exento y total
  por negocio.
- El corte quincenal consolida comisiones y compensaciones por
  `(empresa, negocio)`; `liquidaciones_empresas.negocio_id` permite consultar
  el historial por negocio.
- `finance.series_comprobantes.negocio_id` permite series por negocio; la
  emisión prefiere la serie del negocio de la orden y cae a la transversal.
- Los listados de comprobantes y series aceptan `negocioId`.

### Permisos por acción

- `identity.permisos` versiona las acciones por módulo.
- El rol activo aporta `permisos.acciones`; el JWT incluye la lista y el gateway
  la propaga como `x-user-permisos`.
- Cada servicio valida que el token contenga todas las acciones requeridas;
  desaparece el bypass `rol_empresa_id.is_some()`.
- `crear_rol` rechaza acciones que no existan en el catálogo.
- Las acciones iniciales cubren catálogo, inventario, órdenes, agenda,
  finanzas, logística, personal, tiendas y capacidades.

### RLS por negocio

- El claim firmado incluye `negocio_id` y cada servicio lo inyecta en
  `request.jwt.claims` dentro de la transacción.
- `auth.negocio_actual()` centraliza la lectura del claim.
- Policies `RESTRICTIVE` por tabla: un JWT con negocio activo solo ve filas de
  ese negocio o filas transversales (`negocio_id NULL`).
- Cubiertas: roles, puestos, personal, invitaciones, capacidades de negocio,
  tiendas, ofertas, items, pedidos, comisiones, liquidaciones, comprobantes y
  envíos.
- Sin `negocio_id` en el claim (dueño sin selección) se conserva el alcance de
  toda la empresa.

### Inventario por negocio

- `inventory.ubicaciones`, `inventory.proveedores` y
  `inventory.configuracion_inventario` admiten `negocio_id`.
- La configuración es única por `(empresa, negocio)`; sin fila propia hereda la
  configuración de empresa.
- Ubicaciones y transferencias validan que origen, destino e item pertenezcan al
  mismo negocio.
- Niveles de stock, movimientos y resumen valorizado filtran por negocio.
- RLS restrictiva añadida para ubicaciones, proveedores y configuración.

### Observabilidad y auditoría

- El contexto de logs de todos los servicios incluye `negocio_id` cuando la
  petición lo trae.
- `identity.auditoria` registra cambios de empresa, negocio, rol, invitación y
  capacidad con actor, negocio, request id y datos.
- `GET /api/v2/empresas/{empresaId}/auditoria` filtra por negocio, acción y
  rango de fechas; requiere `auditoria.leer` o rol `empresario`.
- La auditoría queda aislada por RLS de empresa/negocio.
- Las métricas mantienen cardinalidad acotada; el negocio viaja en logs y
  auditoría, no como etiqueta de cada serie.

### Administración de negocios

- `identity.plantillas_roles` versiona roles base por vertical (retail,
  farmacia, belleza, dental, flota).
- Al crear negocio se aplican sus roles base y se crea una ubicación principal
  en inventario.
- `GET /api/v2/negocios/plantillas?vertical_id=` y
  `POST /api/v2/empresas/{id}/negocios/{id}/plantillas` permiten consultar y
  reaplicar plantillas.
- `PATCH /api/v2/empresas/{id}/negocios/{id}/estado` suspende, archiva o
  reactiva sin borrar historial.
- `POST /api/v2/empresas/{id}/personal/{usuario}/transferir` mueve una
  asignación entre negocios validando rol, puesto y tienda.
- Todas las operaciones quedan auditadas.

### Analítica cross-negocio

- La analítica vive en `analytics-service`; `orders-service` queda como fuente de
  hechos y publica `order.completado`.
- El consumidor de `analytics-service` proyecta cada línea vendida en
  `analytics.hechos_venta` (`empresa_id`, `negocio_id`, `comprador_id`,
  cantidades, precios y fecha de la orden) al recibir `order.completado`, y las
  comisiones en `analytics.comisiones` al recibir `comision.acreditada`.
- `GET /api/v2/analytics/negocios` entrega JSON listo para que el
  frontend lo consuma; no renderiza vistas ni dashboard.
- Agrega ventas, órdenes, clientes únicos, comisiones, margen estimado y ticket
  promedio por negocio, más una evolución mensual (`YYYY-MM`).
- Usa `analytics.hechos_venta` para ventas y `analytics.comisiones` para
  comisiones; el filtro por empresa es obligatorio y el negocio activo de la
  sesión prevalece sobre `negocioId`.
- La proyección se alimenta cuando la orden se completa y la línea se comisiona;
  una venta cancelada o compensada no entra en la analítica.
- Acepta `desde`, `hasta` y `formato=csv`; el CSV respeta el mismo desglose.
- Sin dashboard ni componentes visuales: la capa de presentación queda del lado
  del frontend.

### Cohortes y reportes programados

- `GET /api/v2/analytics/negocios/cohortes` agrupa clientes por mes de
  primera compra y devuelve retención por mes índice, con JSON o CSV.
- `analytics.reportes_programados` guarda nombre, tipo (`analitica` o
  `cohortes`), formato, frecuencia, destino, próxima ejecución y estado.
- `analytics.reporte_ejecuciones` conserva el contenido generado y el error,
  si lo hubo, para que el frontend o un notificador lo consuma.
- La API permite listar, crear, pausar, ejecutar ahora y consultar el historial.
- El scheduler de `analytics-service` ejecuta cada 5 minutos los reportes
  vencidos y avanza `proxima_ejecucion` según la frecuencia.
- `POST /internal/analytics/reportes/ejecutar` permite disparar el lote desde
  un orquestador externo.
- La entrega por correo o webhook no se implementa aquí: el backend deja el
  reporte calculado y persistido.

### Relación con `market-intelligence-service`

- `market-intelligence-service` responde preguntas de mercado y operación de
  campo: mapas de calor, cobertura por zona, demanda, tendencias, forecast,
  score de vendedor y calidad de eventos geolocalizados (PostGIS).
- La analítica de negocios responde preguntas de gestión: cuánto vendió cada
  negocio, margen, ticket, evolución y retención de clientes.
- Comparten la palabra “reporte”, pero no el dominio ni el grano: la primera
  trabaja con hechos geoespaciales y predicción; la segunda con líneas de venta
  y contabilidad operativa. Por eso no se fusionan en este ADR.

## Alternativas descartadas

### A. Una empresa por tipo de negocio

Es la opción con menor cambio técnico, pero no cumple el requisito: el dueño
tendría que cambiar de tenant, duplicar el perfil fiscal y no podría compartir
personal ni reportes entre negocios. Se conserva como mecanismo de aislamiento
fuerte para casos legales que realmente lo exijan.

### B. Un microservicio por tipo de negocio

Contradice ADR-0001: la plataforma se compone por capacidades, no por rubro.
Añadir `farmacia-service` o `taller-service` multiplicaría contratos y datos sin
aportar un bounded context distinto.

## Consecuencias

### Positivas

- El dueño conserva una sola empresa, un solo RUC y una sola facturación.
- Los roles y el personal pueden acotarse por negocio sin romper los roles
  transversales existentes.
- La migración es aditiva: los tenants actuales reciben `principal` y siguen
  operando igual.
- La regla de negocio queda explícita y testeada: el segundo negocio exige
  `empresario`.
- `orders-service` vuelve a ser solo fuente de hechos; la analítica y los
  reportes quedan centralizados en `analytics-service`.

### Costes y pendientes

1. **Contexto de negocio en la sesión**: el JWT ya transporta `negocio_id` y el
   gateway lo propaga como `x-negocio`, pero falta un selector explícito de
   negocio para usuarios con varias asignaciones y propagar el contexto a
   tiendas, órdenes y catálogo.
2. **Tiendas y órdenes**: `stores.tiendas.negocio_id` ya existe y se rellena,
   pero `stores-service` y `orders-service` todavía no lo exponen ni filtran por
   él.
3. **Catálogo de permisos**: hoy se reconocen `personal.leer`,
   `personal.gestionar`, `capacidades.leer` y `capacidades.gestionar`; conviene
   publicar un catálogo versionado por módulo.
4. **Consumidores directos**: algunos chequeos internos (por ejemplo la
   validación secundaria de catálogo o la selección de proveedor logístico)
   siguen consultando el resumen de empresa; deben migrarse al contexto de
   negocio de la orden o la tienda.
5. **Entrega de reportes**: el cálculo y el historial ya están resueltos; falta
   conectar correo, webhook o notificaciones para entregar el contenido.

### Sesión y negocio activo

- `identity.sesiones.negocio_id` guarda el negocio elegido por sesión.
- `POST /api/v2/auth/negocio-activo` valida que el negocio pertenezca a la
  empresa y esté asignado al usuario, actualiza la sesión y reemite tokens.
- `POST /api/v2/auth/refresh` conserva el negocio de la sesión.
- `GET /api/v2/auth/negocios` lista los negocios disponibles para el selector.
- Un cambio de negocio no requiere volver a iniciar sesión.

## Fases

| Fase | Entregable | Estado |
|---|---|---|
| 1 | Tabla `negocios`, backfill, `negocio_id` en roles/puestos/personal/invitaciones/tiendas, API de alta y listado | Implementada |
| 2 | `negocio_id` en JWT y `x-negocio`, autorización por rol de negocio y permisos por acción | Implementada |
| 3 | `negocio_capacidades`, endpoint interno con `negocio_id`, `CapabilityGuard` por negocio y API pública | Implementada |
| 4 | Tiendas, órdenes, catálogo e inventario filtrando y reportando por negocio | Implementada |
| 5 | Analítica cross-negocio en `analytics-service` sobre `hechos_venta` (totales, desglose, evolución y CSV) | Implementada |
| 6 | Cohortes y reportes programados en `analytics-service`, con historial de ejecuciones | Implementada |

## Criterios de aceptación de las fases 1-2

- Toda empresa existente tiene exactamente un negocio `principal`.
- Crear una empresa sigue funcionando sin pasos adicionales.
- Una empresa puede crear un segundo negocio con otra vertical y otro pack.
- Un usuario sin asignación activa de `empresario` no puede añadir un negocio:
  el borde responde `ACCESO_DENEGADO` y el dominio lo confirma con
  `EMPRESARIO_REQUERIDO` si la petición llega sin ese filtro.
- Un rol o puesto de un negocio no puede usarse en una invitación de otro
  negocio.
- Una persona invitada con `personal.gestionar` en un negocio puede crear roles
  e invitaciones en ese negocio, pero recibe `ACCESO_DENEGADO` en otro.
- El JWT y el contexto de sesión conservan `negocio_id` tras login y refresh.
- Un negocio con `inventario_lotes` no altera las capacidades de otro negocio
  de la misma empresa.
- El guard interno devuelve el conjunto del negocio cuando recibe `negocio_id`
  y conserva el contrato por empresa cuando no lo recibe.
- Una tienda creada con `x-negocio` queda asignada a ese negocio.
- Las líneas de orden propagan `negocio_id` y la vista lo refleja cuando todas
  las líneas coinciden.
- El checkout valida capacidades por negocio y no por el resumen de empresa.
- Un item creado con `x-negocio` queda en ese negocio, y el listado/stock de
  otro negocio no lo muestra.
- Los movimientos de inventario se filtran por el negocio del item.
- La guía logística se prepara por negocio y el proveedor se deriva de sus
  capacidades.
- El resumen de ventas por negocio cuadra con las líneas de pedido.
- El reporte de comisiones y el libro de ventas agrupan por negocio.
- El negocio activo se puede cambiar sin nuevo login y el refresh lo conserva.
- Un rol configurable solo accede si el token contiene la acción requerida.
- Un rol con permiso inexistente en el catálogo se rechaza.
- Un JWT con negocio activo no puede leer filas de otro negocio vía SQL
  directo; las filas transversales siguen visibles.
- Ubicaciones, proveedores y configuración de inventario respetan el negocio y
  las transferencias no cruzan negocios.
- La emisión fiscal elige una serie del negocio o, si no existe, la transversal.
- Los cambios de negocio, rol e invitación quedan auditados con actor y negocio.
- Los logs incluyen `negocio_id` en peticiones con negocio activo.
- Un negocio nuevo nace con roles base, ubicación principal y capacidad de
  suspenderse/archivarse; el personal puede transferirse entre negocios.
- Los tests de identidad siguen verdes y la migración es reversible por ser
  aditiva.

## Enmienda (2026-11-23)

La **región tributaria** se selecciona a nivel de **empresa**, que sigue siendo
el tenant fiscal ([ADR-0049](./0049-regiones-tributarias.md)). Un negocio no
duplica la elección: hereda la región de la empresa y, si necesita una
jurisdicción distinta, usa el override explícito
`finance.configuracion_fiscal(negocio_id)` que ya define ADR-0026. Cambiar la
región de la empresa no reescribe los overrides por negocio.
