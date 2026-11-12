# Plan de eliminación de compatibilidad hacia atrás

> Contexto: el proyecto **todavía no está en producción**, así que se permite romper
> contratos, esquema y modelos legacy. Este plan ordena la eliminación para que cada
> fase sea verificable y no se mezclen cambios de datos con cambios de API.

## Objetivo

Eliminar todo el código, esquema y contrato que existe únicamente para no romper
versiones anteriores:

- `producto/servicio` → `item`
- `cliente_id` → `parte_id`
- `empresa_miembros/invitaciones_vendedor` → `personal`
- `tipo_empresa/modalidad_logistica` → `vertical_id/pack/capacidades`
- campos y endpoints deprecados
- gates y baselines de contrato que ya no aplican

## Estado de ejecución

**API v2:** completado. Todos los controladores, gateway, smoke, QA y OpenAPI
generado usan `/api/v2`.

**Integración de worktrees:** la rama `docs/auditoria-integral-2026-11-08` y sus
cambios de permisos/capacidades se integraron en `3c687e3`. Se conservó
`/api/v2`, el refactor de `CapabilityGuard` por API interna y las policies RLS
de `orders.orden_vista`; se eliminó la policy de la tabla legacy
`orders.proyeccion_ordenes`.

**Fase 1 (código muerto y aliases):** completada.
- `puedeTransicionar()`/`validarTransicion()` eliminados.
- `orders.proyeccion_ordenes` y `orders.comisiones_vista` eliminadas.
- Endpoint `GET /orders/vendedor/comisiones` eliminado.
- Aliases `reservas_activas` y `carrito.id` eliminados.
- Wrapper `scripts/check-rules.mjs` eliminado.

**Fase 2 (contrato):** completada.
- `producto_id` eliminado de la API de ofertas y de `OfertaResponseDto`.
- Campos de `ProductoResponseDto` y `slug` de empresa/tienda ya no se marcan
  opcionales solo por R-C-7.
- Gate `contract:breaking` y `docs/api/breaking-baseline.json` eliminados.
- OpenAPI regenerado contra `/api/v2`.

**Fase 7 (órdenes sin defaults):** completada.
- `CreateOrderCommand.tipo_orden` es obligatorio.
- `validarTipoOrden()` ya no devuelve `'compra'` por defecto.
- `reconstruirDesdeEventos()` exige `tipo_orden` en el evento.
- Migración `39_orders_sin_defaults.sql` retira el default de
  `orders.orden_vista.tipo_orden`.

**Fase 6 (empresas):** completada.
- `identity.empresas.tipo_empresa` y `modalidad_logistica` eliminadas.
- La logística deriva la flota de capacidades (`logistica_rutas`/`fulfillment_delivery`).
- Catálogo valida `catalogo_items` por API interna, no `tipo_empresa`.

**Fase 4 (partes):** completada.
- `cliente_id` retirado de field/finance/scheduling/logistics/orders.
- `parte_id` es la referencia de cliente; eventos de orden usan `comprador_id`.
- Migración `41_partes_sin_cliente_id.sql`.

**Fase 5 (personal):** completada.
- `identity.empresa_miembros` e `identity.invitaciones_vendedor` eliminadas.
- `asignaciones_personal` es la única fuente de membresías, con
  `participacion_porcentaje`.
- Flujo legacy de invitaciones de vendedores retirado.
- Migración `42_personal_unico.sql`.

**Fase 3 (catálogo):** completada.
- `catalog.items` es la única fuente: productos, servicios y alquileres viven en
  la misma tabla y las variantes usan `parent_item_id`.
- Las imágenes se guardan en `catalog.item_imagenes`; `productos`,
  `servicios` y `producto_imagenes` fueron retiradas junto con sus triggers de
  sincronización (migraciones `42a`, `44` y `48`).
- Las operaciones (combos, promociones, ajustes, histórico) referencian
  `item_id`; `stores.ofertas` ya no conserva `producto_id`.
- Los endpoints legacy `/catalog/productos*` y `/catalog/servicios*` se
  retiraron; la edición del catálogo vive en `/catalog/items` y los endpoints
  internos solo exponen `/internal/items/:id` y `/internal/items/lote`.
- OpenAPI regenerado y smoke alineado con la superficie de items.

---

## Decisión de la Fase 3

Variantes como items hijos con `parent_item_id` e imágenes en
`catalog.item_imagenes`. La reescritura del catálogo quedó ejecutada; `items`
es la única fuente de verdad.

---

## Estrategia de esquema

Hay dos caminos:

| Opción | Cuándo usarla | Coste |
|---|---|---|
| **A. Migraciones destructivas + squash final** (recomendada) | Quieres trazabilidad mientras se limpia | Más PRs, pero cada paso es reversible y auditable |
| **B. Editar `infra/db/init` y resetear la BD** | Nadie tiene datos que conservar y se puede borrar el volumen | Menos PRs, pero se pierde el historial de migraciones |

Este plan asume **A**. Al final de la fase 9 se hace el squash a un baseline limpio
y se regenera `scripts/config/db-migrations.json`.

---

## Fase 0 — Preparación

**Objetivo:** congelar una foto del estado actual antes de romper nada.

- [ ] Crear rama `chore/drop-backward-compatibility`.
- [ ] Verificar el estado base:

```bash
npm ci
npm run build
npm test
npm run lint
npm run rules:check
npm run swagger:export
```

- [ ] Guardar una copia del OpenAPI actual en `docs/api/` (ya está versionado).
- [ ] Confirmar que no hay consumidores externos de `/api/v1` (apps móviles,
      frontend desplegado, integraciones). Si hay un frontend, acordar la ventana
      de despliegue conjunto.
- [ ] Decidir si se conserva el historial de slugs (recomendado: **sí**; es una
      funcionalidad de producto, no solo compatibilidad).

**Criterio de salida:** build, tests y rules en verde antes de tocar código.

---

## Fase 1 — Borrar código muerto y aliases triviales

**Riesgo:** bajo. No cambia comportamiento de negocio.

### 1.1 Eliminar transiciones legacy

- [ ] `packages/shared/src/order-state.ts`
  - Borrar `puedeTransicionar()` y `validarTransicion()`.
  - Dejar solo `validarTransicionPorTipo()` y `TRANSICIONES_POR_TIPO`.
- [ ] Actualizar:
  - `packages/shared/src/order-state.spec.ts`
  - `packages/shared/src/order-state.property.spec.ts`
- [ ] Buscar usos:

```bash
grep -rn "puedeTransicionar\|validarTransicion\b" packages scripts
```

### 1.2 Eliminar tablas/proyecciones muertas

- [ ] `orders.proyeccion_ordenes`:
  - Quitar de `infra/db/init/01_esquemas.sql`.
  - Quitar de `infra/db/supabase/99_rls.sql`.
  - Quitar de `qa-harness/src/suites/17-schema.js`.
- [ ] `orders.comisiones_vista`:
  - Quitar el endpoint `GET /api/v1/orders/vendedor/comisiones`
    (`packages/orders-service/src/pedidos/pedidos.controller.ts:71-96`).
  - Quitar `OrderViewRepository.obtenerComisiones()` y el tipo `ComisionVista`
    (`order-view.repository.ts:208-224`).
  - Quitar `ComisionVistaResponseDto` y sus tests.
  - Quitar la agregación de comisiones del trigger
    `orders.fn_aplicar_evento` en `infra/db/init/01d_es_orders.sql`.
  - Quitar la tabla `orders.comisiones_vista` de `01_esquemas.sql`/`01d`.
- [ ] Quitar `orders.proyeccion_ordenes` y `comisiones_vista` del RLS y del QA.

### 1.3 Eliminar aliases de respuesta

- [ ] `LineaInventarioResponseDto.reservas_activas`
  (`packages/catalog-service/src/inventario/inventario.response.dto.ts:36-37`).
- [ ] `CarritoItemResponseDto.id` alias de `oferta_id`
  (`packages/orders-service/src/pedidos/carrito/carrito.response.dto.ts:10`).
- [ ] Actualizar specs y regenerar OpenAPI.

### 1.4 Eliminar el wrapper de reglas

- [ ] Borrar `scripts/check-rules.mjs`.
- [ ] Cambiar `package.json`:
  - `"rules:check": "node scripts/cli.mjs rules"`
- [ ] Actualizar referencias en `scripts/README.md`, docs y CI.

**Criterio de salida:**

```bash
npm run build && npm test && npm run rules:check
npm run swagger:export && git diff --exit-code docs/openapi docs/api packages/api-gateway/src/openapi.agregado.ts
```

---

## Fase 2 — Cerrar el contrato API

**Riesgo:** medio. Rompe clientes que usen campos/endpoints legacy.

### 2.1 Quitar `producto_id` de publicación de ofertas

- [ ] `packages/stores-service/src/ofertas/ofertas.controller.ts`
  - Borrar `PublicarProductoRequestDto.producto_id`.
  - Dejar solo `item_id` + `item_type` (o `item_id` obligatorio y tipo inferido).
  - Borrar la rama `publicar()`; queda solo `publicarItem()`.
- [ ] `packages/stores-service/src/ofertas/ofertas.service.ts`
  - Borrar `publicar()`.
  - Borrar `producto_id`, `producto_padre_id`, `sku`, `producto_nombre` de
    `Oferta`/`OfertaRow`/`serializar()` si se migran a `item_id` + join a items.
- [ ] `packages/stores-service/src/ofertas/ofertas.response.dto.ts`
  - Quitar `producto_id` y `producto_padre_id`.
- [ ] `packages/orders-service`
  - `OfertaExterna.producto_id` puede desaparecer.
  - `item_id: oferta.item_id ?? oferta.producto_id` → `item_id: oferta.item_id`.
  - `item_type ?? 'producto'` → `item_type` obligatorio.
- [ ] Tests: `ofertas.controller.spec.ts`, `ofertas.service.spec.ts`,
  `create-order-command.handler.spec.ts`.

### 2.2 Quitar campos opcionales “solo por R-C-7”

- [ ] `ProductoResponseDto`: quitar los `required: false` de metadatos
  (`packages/catalog-service/src/productos/productos.response.dto.ts:89-174`).
- [ ] `EmpresaResponseDto.slug` → obligatorio
  (`identity-service/src/empresas/empresas.controller.ts:130-132`).
- [ ] `TiendaResponseDto.slug` → obligatorio
  (`stores-service/src/tiendas/tiendas.response.dto.ts:111-114`).
- [ ] `OfertaResponseDto.item_type/item_id` → obligatorios.
- [ ] `OrderViewResponseDto.tipo_orden/parte_id/fulfillment_config` → obligatorios
  (tras fases 4 y 7).
- [ ] `CitaResponseDto.parte_id` → obligatorio (tras fase 4).
- [ ] `ComprobanteResponseDto.parte_id` → obligatorio (tras fase 4).

### 2.3 Endpoint deprecado

- [ ] Ya eliminado en Fase 1.2. Verificar que no quede en:
  - `docs/api/core-engine.openapi.json`
  - `docs/openapi/orders-service/public.openapi.json`
  - `packages/api-gateway/src/openapi.agregado.ts`

### 2.4 Desactivar el gate de cambios incompatibles

Mientras el proyecto no tenga clientes externos, el gate R-C-7 solo estorba:

- [ ] Quitar el job `Gate R-C-7` de `.github/workflows/ci.yml`.
- [ ] Borrar `scripts/tools/contract-breaking.mjs` y `scripts/engine/lib/openapi-diff.mjs`.
- [ ] Borrar `docs/api/breaking-baseline.json`.
- [ ] Quitar `contract:breaking` de `package.json`.
- [ ] Actualizar `docs/rules/test/04-reglas-contrato.md`:
  - R-C-6/R-C-7 se reemplazan por “el contrato se regenera y se revisa en el PR”.
- [ ] Mantener `swagger:export` + `git diff --exit-code` si quieres que el spec
      siga sincronizado; es barato y no obliga a compatibilidad.

**Criterio de salida:** no queda ningún `deprecated: true`, `Deprecation`, ni
`producto_id` en el contrato público.

---

## Fase 3 — Unificar catálogo: `items` como única fuente

**Riesgo:** alto. Es la fase más grande.

### 3.1 Migrar datos y escrituras

- [ ] Backfill definitivo de `catalog.productos` y `catalog.servicios` a
  `catalog.items` (ya existe en `25_catalog_items.sql`).
- [ ] Migrar variantes e imágenes:
  - `catalog.productos.producto_padre_id` → relación de items o tabla
    `catalog.item_variantes`.
  - `catalog.producto_imagenes` → `catalog.item_imagenes` (o mantener tabla pero
    con `item_id`).
- [ ] Migrar operaciones que referencian `producto_id`:
  - combos, promociones, ajustes de stock, historial de precios
    (`catalog-service/src/operaciones`, SQL `12_catalogo_operaciones_avanzadas.sql`).
- [ ] Migrar `stores.ofertas.producto_id` → `item_id` (Fase 2 ya quitó el campo de API).
- [ ] Migrar `stock_vista`, `producto_catalogo_vista`, `inventario_mensual_mv` a
  claves basadas en `item_id`/`empresa_id`.

### 3.2 Cambiar escrituras de producto/servicio a `items`

- [ ] `catalog-service/src/productos/productos.service.ts`:
  - Crear/actualizar en `catalog.items` con `tipo='producto'`.
- [ ] `catalog-service/src/servicios/servicios.service.ts`:
  - Crear/actualizar en `catalog.items` con `tipo='servicio'`.
- [ ] `catalog-service/src/items/items.service.ts`:
  - Aceptar `tipo` producto/servicio/alquiler y validar `duration_minutes`,
    `track_inventory`, `stock`.
- [ ] Eliminar los triggers `trg_sync_item_producto`/`trg_sync_item_servicio`.

### 3.3 Cambiar lecturas

- [ ] `productos.service.ts` y `servicios.service.ts` leen de `catalog.items_vista`.
- [ ] `inventario.service.ts` lee de items + movimientos/lotes.
- [ ] `internal.controller.ts`:
  - Quedan solo `/internal/items/:id` y `/internal/items/lote`.
  - Eliminar `/internal/productos/*`.
- [ ] `stores-service` consulta solo `/internal/items/:id`.
- [ ] `orders-service` valida stock solo por `item_id`/`empresa_id`.

### 3.4 Retirar endpoints y tablas legacy

- [ ] Eliminar `GET/POST/PATCH/DELETE /api/v1/catalog/productos*`.
- [ ] Eliminar `GET/POST/PATCH/DELETE /api/v1/catalog/servicios*`.
- [ ] Redirigir o documentar el reemplazo en `/api/v1/catalog/items`.
- [ ] Añadir migración destructiva:
  - `DROP TABLE catalog.productos CASCADE;`
  - `DROP TABLE catalog.servicios CASCADE;`
  - `DROP TABLE catalog.producto_imagenes;`
  - `DROP VIEW catalog.producto_catalogo_vista;`
  - Limpiar funciones/triggers asociados.
- [ ] Quitar `source_type`/`source_id` de `catalog.items`.
- [ ] Quitar `EMPRESA_LEGACY_ID` y la empresa “Catálogo legado” si ya no hay filas.

**Criterio de salida:**

```bash
grep -rn "producto_id\|catalog.productos\|catalog.servicios\|producto_catalogo_vista" packages infra/db/init
```

Solo deben quedar referencias históricas en migraciones antiguas (o ninguna si se
hace squash).

---

## Fase 4 — Unificar clientes: `parte_id` como única referencia

**Riesgo:** alto. Toca 5 servicios y eventos.

### 4.1 Migrar datos

- [ ] Backfill definitivo de `field.clientes` → `identity.partes` (ya existe).
- [ ] Backfill de `finance.workspace_registros` (`recurso='clientes'`) → partes.
- [ ] Migrar `finance.comprobantes.cliente_id` → `parte_id`.
- [ ] Migrar `scheduling.citas.cliente_id` → `parte_id`.
- [ ] Migrar `logistics.envios` → `parte_id`.
- [ ] Migrar `orders.orden_vista.comprador_id` → `parte_id` (mantener `comprador_id`
      solo si se necesita el usuario autenticado, no el cliente).
- [ ] Migrar payloads de eventos:
  - `cliente_id` → `cliente_parte_id` (o `parte_id`) obligatorio.

### 4.2 Cambiar servicios

- [ ] `field-service`:
  - Eliminar `clienteId` de DTOs.
  - Dejar solo `parteId`; `cliente_id` desaparece del esquema.
  - Migrar `field.clientes` a una vista temporal de `identity.partes` o eliminarla.
- [ ] `finance-service/facturacion`:
  - Quitar `cliente_id`, dejar `parte_id`.
  - `datos_cliente` JSON se mantiene solo como snapshot fiscal, no como referencia.
- [ ] `scheduling-service`:
  - Quitar `cliente_id` de DTO/tabla/response/evento.
- [ ] `logistics-service`:
  - Quitar `cliente_id` del contrato externo.
  - Corregir el desalineamiento actual: orders devuelve `comprador_id`, logistics
    tipa `cliente_id`. Con `parte_id` único desaparece el problema.
- [ ] `orders-service`:
  - `CreateOrderCommand.cliente_parte_id` → obligatorio.
  - `OrderView.parte_id` obligatorio; `comprador_id` queda solo como usuario.
- [ ] `identity-service/partes`: validar que sea la única API de clientes.

### 4.3 Retirar legado

- [ ] Quitar `parte_id` nullable y `cliente_id` de las 9 tablas de
  `36_partes_compatibilidad.sql`.
- [ ] Quitar `metadata.legacy_id` y `metadata.origen` si ya no se necesita trazabilidad.
- [ ] Quitar `orders.fn_sync_order_part` si `parte_id` ya viene en el evento principal.
- [ ] Eliminar `field.clientes` o convertirla en vista de solo lectura.
- [ ] Eliminar `finance.workspace_registros` para `recurso='clientes'` (o toda la
  tabla si los demás recursos también se migran).

**Criterio de salida:**

```bash
grep -rn "cliente_id\|clienteId" packages/*/src infra/db/init
```

Solo debe quedar `usuario.cliente` conceptual o `comprador_id` como usuario
autenticado, nunca como referencia de parte.

---

## Fase 5 — Unificar personal: eliminar `empresa_miembros` e `invitaciones_vendedor`

**Riesgo:** alto. Toca autenticación, JWT, gateway y autorización.

### 5.1 Migrar datos

- [ ] Migrar `identity.empresa_miembros` a:
  - `identity.asignaciones_personal` + `identity.roles_empresa`.
- [ ] Migrar `identity.invitaciones_vendedor` a `identity.invitaciones_personal`.
- [ ] Migrar `usuarios.rol` y `empresa_miembros.rol` a roles por empresa.
- [ ] Decidir qué pasa con `participacion_porcentaje`: moverlo a
  `asignaciones_personal.metadata` o a una tabla de participación.

### 5.2 Cambiar servicios

- [ ] `identity-service/src/empresas/empresas.service.ts`
  - `crearVendedor()` y `POST /empresas/:id/vendedores` se reemplazan por
    `asignaciones-personal`.
- [ ] `identity-service/src/invitaciones/`
  - Eliminar todo el módulo legacy.
  - Dejar solo `personal/invitaciones` + `incorporaciones`.
- [ ] `identity-service/src/personal/`
  - Dejar de escribir en `empresa_miembros` (ya no existirá).
  - Ser la única fuente de roles/puestos/asignaciones.
- [ ] `auth.service.ts` y `shared/src/auth/jwt.utils.ts`
  - Reemplazar `rol` global por `rol_empresa_id`/permisos.
- [ ] Gateway:
  - Reemplazar políticas basadas en `ROLES.*` por capacidades/permisos.
  - Eliminar rutas legacy de invitaciones de vendedores.

### 5.3 Retirar legado

- [ ] `DROP TABLE identity.invitaciones_vendedor;`
- [ ] `DROP TABLE identity.empresa_miembros;`
- [ ] Quitar `empresa_miembros_rol_compatible_check`.
- [ ] Quitar `rol` de `identity.usuarios` si ya no aplica.
- [ ] Quitar `ROLES.VENDEDOR`/`EMPRESARIO` como roles globales si el modelo pasa a
  roles por empresa.

**Criterio de salida:** solo existe `personal` como flujo de alta y autorización.

---

## Fase 6 — Eliminar `tipo_empresa` y `modalidad_logistica`

**Riesgo:** medio-alto. Afecta alta de empresas, catálogo y logística.

- [ ] Migrar `identity.empresas.tipo_empresa` → `vertical_id` + `pack_codigo`.
- [ ] Migrar `identity.empresas.modalidad_logistica` → capacidades
  `fulfillment_*` + `fulfillment_config`.
- [ ] Migrar `logistics.envios.modalidad_logistica` → configuración por capacidades.
- [ ] Quitar de `Empresa`/`EmpresaFila`/DTOs:
  - `tipo_empresa`
  - `modalidad_logistica`
- [ ] Quitar endpoints:
  - `PATCH /api/v1/empresas/:id/logistica`
  - `PATCH /api/v1/empresas/:id/tipo`
- [ ] Hacer obligatorios `vertical_id`/`pack_codigo` en el alta.
- [ ] Quitar el fallback:
  - `vertical = verticalId ?? (tipo === 'logistica' ? 'flota' : 'retail')`
  - `pack = { codigo: 'pack_${vertical}', version: 1 }`
- [ ] `catalog-service`: validar por capacidad `catalogo_items` en vez de
  `tipo_empresa === 'comercial'`.
- [ ] Migración destructiva:
  - `ALTER TABLE identity.empresas DROP COLUMN tipo_empresa;`
  - `ALTER TABLE identity.empresas DROP COLUMN modalidad_logistica;`
  - `ALTER TABLE logistics.envios DROP COLUMN modalidad_logistica;`
- [ ] Quitar `tipo_empresa` del `openapi` y de `qa-harness` si lo referencia.

**Criterio de salida:** una empresa se define solo por `vertical_id`, `pack_*`,
capacidades y `fulfillment_config`.

---

## Fase 7 — Órdenes sin defaults legacy

**Riesgo:** medio.

- [ ] `CreateOrderCommand.tipo_orden` obligatorio.
- [ ] `CreateOrderCommand.items` obligatorio (o `usar_carrito` explícito).
- [ ] `ItemOrden.item_type` e `item_id` obligatorios.
- [ ] Quitar `tipo_orden ?? 'compra'` en:
  - `models/order.ts`
  - `order-view.repository.ts`
  - `create-order-command.handler.ts`
- [ ] Quitar `fulfillment ?? 'none'` o hacerlo explícito en el contrato.
- [ ] Normalizar eventos:
  - `OrderCreatedEvent` vs `order.created`: usar un solo nombre.
  - `OrderStatusUpdatedEvent` vs `order.status.updated`: idem.
- [ ] Quitar el bootstrap que rellena `tipo_orden`/`fulfillment_config` con defaults.
- [ ] Eliminar `orders.comisiones_vista` (Fase 1) y confirmar que
  `commissions-service` es la única fuente de comisiones.

**Criterio de salida:** no hay `?? 'compra'`, `?? 'producto'` ni `?? 'none'` en
producción.

---

## Fase 8 — Slugs: decidir explícitamente

**Recomendación: conservar el historial de slugs.** Es una funcionalidad de
producto (enlaces estables) y no impone compatibilidad de esquema.

Si aun así se quiere eliminar:

- [ ] Borrar `identity.empresa_slugs`, `stores.tienda_slugs`, triggers y backfill
  (`01f_slugs.sql`).
- [ ] Quitar `buscarPorIdentificador()`, `resolver()` y `redirigirA` de
  `identity/empresas.service.ts` y `stores/tiendas.service.ts`.
- [ ] Quitar los 301 de los controllers.
- [ ] Hacer `slug` inmutable o eliminar el endpoint `PATCH /slug`.
- [ ] Hacer `slug` obligatorio en los DTOs (ya se hizo en Fase 2).
- [ ] Eliminar los tests de slug anterior y el smoke de 301.

---

## Fase 9 — Limpieza final y squash

### 9.1 Gobierno

- [ ] Revisar `docs/rules/`:
  - R-C-6/R-C-7: eliminar o reescribir.
  - R-GW-7: mantener `/api/v1` solo si se quiere versionar desde el inicio.
  - R-DB-5/R-DB-6: decidir si se conserva el manifiesto de migraciones.
- [ ] Eliminar `scripts/config/db-migrations.json` si se hace squash.
- [ ] Eliminar `scripts/tools/baseline-guard.mjs` si ya no hay baselines.
- [ ] Eliminar `scripts/config/release.mjs` / `VERSION_MATCH_FROM` si se resetea la
  numeración de versiones.
- [ ] Revisar `.github/dependabot.yml`:
  - Quitar el `ignore` de NestJS cuando `@nestjs/swagger` soporte la versión actual.
- [ ] Revisar `package.json`:
  - Quitar overrides/vendor pins si upstream ya resuelve.

### 9.2 Squash de migraciones

Cuando todo esté estable:

- [ ] Crear un baseline limpio de `infra/db/init/` (por ejemplo `00_baseline.sql`
  más migraciones de features).
- [ ] Eliminar las migraciones aditivas intermedias.
- [ ] Regenerar:

```bash
npm run db:migrations -- --update
```

- [ ] Resetear la BD local/stage y validar desde cero:

```bash
docker compose -f docker-compose.dev.yml down -v
docker compose -f docker-compose.dev.yml up -d
npm run test:integration
```

### 9.3 Tests y QA

- [ ] Eliminar tests exclusivos de compatibilidad:
  - `scripts/integration/orders-types.test.mjs` (“sin romper compras legacy”).
  - `scripts/integration/catalog-items.test.mjs` (“legacy se proyectan”).
  - specs de `EMPRESA_LEGACY_ID`.
  - specs de slug anterior si se elimina el historial.
  - specs del endpoint deprecado.
  - `qa-harness` expectations de tablas legacy.
- [ ] Actualizar `qa-harness/src/suites/17-schema.js` con solo las tablas nuevas.
- [ ] Actualizar `scripts/tools/smoke.mjs` para quitar pasos de 301 si aplica.

### 9.4 Documentación

- [ ] Actualizar `docs/adr/0001-plataforma-multi-vertical.md`:
  - Ya no hay “compatibilidad progresiva”; hay modelo único.
- [ ] Actualizar `docs/runbook-multi-vertical.md`:
  - Quitar el “flag de compatibilidad legacy”.
- [ ] Actualizar `README.md` y `docs/decisiones.md` con el estado final.

---

## Orden de PRs sugerido

| PR | Fase | Alcance | Riesgo |
|---|---|---|---|
| 1 | 1 | Código muerto, aliases, wrapper | Bajo |
| 2 | 2 | Contrato: `producto_id`, campos opcionales, endpoint deprecado, gate R-C-7 | Medio |
| 3 | 3.1 | Datos: variantes/imágenes/operaciones a `item_id` | Alto |
| 4 | 3.2–3.4 | Escrituras/lecturas a `items` + drop de tablas legacy | Alto |
| 5 | 4.1–4.2 | `parte_id` en field/finance/scheduling/logistics/orders | Alto |
| 6 | 4.3 | Drop de `cliente_id` y tablas JSON legacy | Alto |
| 7 | 5 | Personal universal + drop `empresa_miembros`/`invitaciones_vendedor` | Alto |
| 8 | 6 | Drop `tipo_empresa`/`modalidad_logistica` | Medio-alto |
| 9 | 7 | Órdenes sin defaults legacy | Medio |
| 10 | 8 | Slugs (solo si se decide eliminar) | Medio |
| 11 | 9 | Squash de migraciones, tests y docs | Bajo |

## Verificación transversal

Después de cada PR:

```bash
npm run build
npm test
npm run lint
npm run rules:check
npm run swagger:export && git diff --exit-code docs/openapi docs/api packages/api-gateway/src/openapi.agregado.ts
```

Con BD:

```bash
npm run test:integration
npm run multi-vertical:check
npm run db:rls
```

Si se tocó despliegue:

```bash
npm run promotion:check
npm run smoke:remote
```

## Definición de terminado

- No existe ningún `legacy`, `compat`, `deprecated`, `obsoleto` ni `fallback`
  justificado solo por versiones anteriores.
- No existen columnas `cliente_id`/`producto_id` ni tablas `empresa_miembros`,
  `invitaciones_vendedor`, `catalog.productos`, `catalog.servicios`.
- No existen endpoints duplicados ni campos alias.
- El esquema se levanta desde cero y las integraciones pasan.
- El OpenAPI se regenera sin excepciones de breaking changes.
