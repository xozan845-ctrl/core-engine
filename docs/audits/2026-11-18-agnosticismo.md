# Auditoría de agnosticismo multi-negocio — Core Engine

- **Fecha:** 2026-11-18
- **Base auditada:** `develop` @ `9b23461a` (árbol limpio, sincronizado con `origin/develop`).
- **Método:** lectura estática del código Rust, migraciones SQL, contratos OpenAPI y
  pruebas existentes. No se ejecutó la suite completa ni un stack real; por eso
  cada afirmación lleva archivo y línea para que pueda reproducirse.
- **Alcance:** las 7 afirmaciones del diagnóstico inicial y la tabla de madurez
  por dimensión. El objetivo es saber **qué es cierto, qué está exagerado y qué
  falta**, antes de invertir en el MVP.

## 1. Veredicto

El diagnóstico inicial **acierta en la dirección y en la mayoría de los
hallazgos**, pero mezcla tres cosas distintas:

1. **Limitaciones reales de diseño** (marketplace en órdenes/comisiones, fiscal
   Nicaragua-céntrico, promociones inertes, ubicación no elegible).
2. **Detalles imprecisos** (las tiendas ya no exigen vendedor en la base; el
   stock no descuenta “la primera activa”, reparte entre ubicaciones; el gating
   de capacidades no es uniforme).
3. **Deuda funcional que no es de agnosticismo** (pagos simulados, ausencia de
   RMA parcial, cupones, reseñas/wishlist), aunque sí afecta a qué negocios se
   pueden atender.

La conclusión de fondo se mantiene: **Core Engine es multi-tenant y
multi-vertical, pero no es todavía multi-modelo de negocio**. Sabe aislar
empresas y activar módulos, pero el flujo comercial “vender” está modelado como
marketplace (tienda → vendedor → comisión), no como venta directa de una
empresa.

## 2. Tabla corregida

| Dimensión | Nivel real | Qué corrige la tabla original |
|---|---|---|
| Tenancy (`empresa` → `negocio`) | **Alto** | Confirmado. RLS por empresa y por negocio, JWT con `tenant_id`/`negocio_id`, capacidades por negocio y packs por empresa. |
| Verticales y packs | **Alto, con condición** | Se crean por API, pero **solo reutilizando capacidades y modelos existentes**. Un vertical nuevo que necesite una capacidad nueva vuelve al punto 2. |
| Capacidades existentes | **Medio/Alto** | La activación por empresa/negocio y el manifiesto existen. El “gating uniforme” **no**: cada servicio monta su guard y varios no lo montan. |
| Capacidad nueva | **Bajo** | Confirmado: exige migración, `features.rs`, guard, endpoints y eventos. No hay endpoint para crear capacidades. |
| Catálogo e inventario | **Alto/Medio** | Confirmado: ítems genéricos, variantes, lotes FEFO, multi-ubicación. Las reglas comerciales finas (impuestos por línea, sustituciones, disponibilidad por canal) no están. |
| Precios | **Medio/Alto** | Confirmado: listas, segmentos, escalas, tarifas por fecha. Las promociones son CRUD; **no se aplican en checkout**. |
| Órdenes | **Medio** (para agnosticismo: **Medio/Bajo**) | CQRS/ES, tipos y transiciones configurables y fulfillments son sólidos. El acoplamiento a vendedor/tienda/comisión es estructural, no configurable. |
| Fiscal | **Bajo** | Más grave de lo que sugería “Bajo/Medio”: la jurisdicción activa se elige **globalmente por orden alfabético**, la moneda del comprobante está fija en `C$` y el IVA se calcula sobre el total de la orden, no por línea. |
| Logística y campo | **Medio** | No se auditó en profundidad en este trabajo; la evidencia disponible (envíos, flotas, rutas, tracking, incidencias) respalda el nivel medio. |

## 3. Punto por punto

### 3.1 “El marketplace está en el ADN” — **Confirmado, con un matiz importante**

**Lo cierto.** La orden no puede existir sin vendedor y tienda:

- `orders.orden_vista.vendedor_id` y `tienda_id` son `NOT NULL`
  (`infra/db/init/01d_es_orders.sql:22` y `:24`).
- El contrato de la oferta que consume orders exige ambos como `String`
  (`packages/orders-service/src/clients.rs:56-57`), igual que `ItemOrden`
  (`packages/orders-service/src/order_state.rs:32-33`) y la vista de salida
  (`packages/orders-service/src/views.rs:23,26`).
- La comisión se calcula por línea y se reparte entre vendedor y empresario
  (`packages/commissions-service/src/comisiones.rs:101-126`), con la
  participación que viene de la tienda/oferta.

**El matiz.** Las tiendas ya **no** exigen vendedor en la base:

- `stores.tiendas.vendedor_id` pasó a nullable
  (`infra/db/init/01g_invitaciones_vendedor.sql:7`) y la tienda puede publicar
  por `propietario_usuario_id` o por personal asignado
  (`packages/stores-service/src/ofertas/mod.rs:399-416`).
- Pero si la tienda no tiene vendedor, la oferta sale con `vendedor_id: null` y
  **orders no puede deserializarla**; además la proyección no admite `NULL`.
  Es decir: la base permite el caso, el flujo comercial no.

**Consecuencia.** Una empresa única puede operar si crea una tienda y asigna a
su dueño como vendedor. No es un modo soportado: es un marketplace con un solo
vendedor. La comisión sigue existiendo y **no puede ser 0** (ver 3.8).

### 3.2 “La configuración pura tiene límites” — **Confirmado**

- El mapa capacidad → features es una constante cerrada
  (`packages/identity-service/src/dominio/features.rs:10-120`); una capacidad
  desconocida devuelve `[]` (`:122-128`).
- Las capacidades se siembran por migración; no hay `POST /capacidades` en las
  rutas de identity (solo `GET`/`PATCH` sobre empresas y negocios,
  `packages/identity-service/src/http/mod.rs:274-291`).
- Crear un vertical o un pack por API sí es posible
  (`packages/identity-service/src/http/mod.rs:293-302`), pero solo con
  capacidades/modelos ya existentes
  (`packages/identity-service/src/dominio/packs.rs:290-330`).

**Matiz.** “features.rs y los guards son listas cerradas” es correcto para
features; los guards son otra historia (ver 3.9).

### 3.3 “Promociones no operan” — **Confirmado**

- Existe el CRUD y los cuatro tipos: `2x1`, `descuento_cantidad`,
  `descuento_porcentaje` y `regalo`
  (`infra/db/init/12_catalogo_operaciones_avanzadas.sql:42-63`).
- El servicio solo lista/crea/edita/elimina
  (`packages/pricing-service/src/promociones/mod.rs`).
- Ni `orders-service` ni `stores-service` consultan `pricing.promociones`; el
  checkout solo resuelve precio base, lista/segmento, escalas y tarifas
  (`packages/orders-service/src/checkout.rs:224-232` y `:797-812`).

**Consecuencia.** La promoción es configuración visible pero no cambia el
precio real. Para el MVP, o se aplica en el cálculo de precios o se retira de la
promesa comercial.

### 3.4 “Multi-ubicación no decide” — **Confirmado en el fondo, impreciso en el detalle**

- El checkout no envía `ubicacion_id`; inventory tampoco lo pide para reservar.
- La reserva valida el stock **sumando todas las ubicaciones**
  (`packages/inventory-service/src/reservas.rs:38-43`).
- Al descontar, `stock.rs` prioriza la ubicación predeterminada y, si no
  alcanza, reparte por mayor stock (`packages/inventory-service/src/stock.rs:82-100`).
  No es “la primera activa”: es predeterminada → mayor stock → orden estable.
- Los lotes FEFO se consumen por vencimiento, sin ubicación
  (`packages/inventory-service/src/reservas.rs:80-120`).

**Consecuencia.** El stock es correcto a nivel agregado, pero la promesa “despacha
desde la bodega elegida” no existe. Para una distribuidora con varias bodegas,
esto es un bloqueador de negocio, no un detalle.

### 3.5 “Fiscal Nicaragua-céntrico” — **Confirmado y más profundo**

- Semilla de Nicaragua con Ley 822, IVA 15 %, IR 30 %, moneda `C$`
  (`infra/db/init/01_esquemas.sql:613-620`).
- `finance.comprobantes.moneda` tiene default `C$` y la emisión lo escribe
  literalmente en el `INSERT` (`infra/db/init/01_esquemas.sql:493`,
  `packages/finance-service/src/facturacion.rs:267`).
- La declaración usa la **primera jurisdicción activa ordenada por código de
  país**, no la del tenant (`packages/finance-service/src/tributacion.rs:149-153`).
  Con dos jurisdicciones activas, el resultado depende del alfabeto.
- El IVA se calcula sobre el total de la orden
  (`packages/finance-service/src/facturacion.rs:241-251`); no hay impuestos por
  línea en orders/pricing.
- No hay facturación electrónica DGI ni generación de PDF: se emite un
  comprobante interno (`BORRADOR`/`EMITIDO`) con serie y secuencial.

**Consecuencia.** “Jurisdicciones configurables” es cierto a nivel de tabla,
pero el comportamiento fiscal sigue siendo de una sola jurisdicción global.
Esto no soporta una empresa con operaciones en dos países.

### 3.6 “Pagos simulados” — **Confirmado**

- Rutas disponibles: métodos de pago, confirmación/rechazo manual y webhook
  (`packages/orders-service/src/http/pagos.rs:18-28`).
- No hay creación de cargo, conciliación, reembolso ni chargeback. El estado
  `reembolsado` existe en el `CHECK` de `orders.pagos`
  (`infra/db/init/77_metodos_pago.sql:33-35`) pero no hay transición que lo
  produzca.
- El rechazo de un pago cancela la orden y emite `devolucion.solicitada`
  (`packages/orders-service/src/http/pagos.rs:217-246`), pero no reembolsa
  dinero.
- El pago de comisiones es explícitamente simulado
  (`packages/commissions-service/src/comisiones.rs:270-280`).

### 3.7 “Supuestos de comercio” — **Confirmado, con una corrección**

- No hay impuestos por línea: el total es suma de precio × cantidad
  (`packages/orders-service/src/checkout.rs:224-232`).
- No hay RMA ni devolución parcial. Sí hay devolución **total** por rechazo de
  pago o transición a `devuelta`, con compensación de comisión
  (`packages/orders-service/src/order_state.rs:49-63`,
  `packages/commissions-service/src/comisiones.rs:211-263`).
- No hay cupones, reseñas ni wishlist (no aparecen tablas, rutas ni servicios).
- No hay reembolso parcial ni notas de crédito asociadas a una devolución
  parcial; `NOTA_CREDITO` existe como tipo de comprobante, pero el flujo es
  manual.

## 4. Hallazgos adicionales (no estaban en el diagnóstico)

### 4.1 La comisión no puede ser 0 y la respuesta puede mentir

- `money::comision` exige `0 < tasa < 1`; con `COMMISSION_RATE=0` el cálculo
  falla (`packages/orders-service/src/money.rs:87-95`). La configuración
  también filtra `> 0` (`packages/orders-service/src/config.rs:113-120`).
- En el mismo checkout, el evento usa `self.tasa_comision`
  (`packages/orders-service/src/checkout.rs:446-450`), pero la vista HTTP
  devuelve `money::comision(total, 0.12)` **hardcodeado**
  (`packages/orders-service/src/checkout.rs:521-524`). Con una tasa distinta de
  12 %, la respuesta y el evento no coinciden.
- `commissions-service` replica la misma restricción `0 < tasa < 1`
  (`packages/commissions-service/src/money.rs:25-34`).

**Impacto:** una empresa de venta directa no puede desactivar la comisión; y si
cambia la tasa, el cliente ve un número distinto del que se contabiliza.

### 4.2 El gating de capacidades no es uniforme

Hay guard y consulta de capacidades en orders, commissions, finance, field,
inventory, item, logistics, market-intelligence, analytics, pricing y stores.
No aparece, con el mismo patrón, en notifications, storage ni identity. La
auditoría integral anterior ya lo había documentado como “CapabilityGuard
parcial” (`docs/audits/2026-11-08-revision-integral.md`, sección 5).

**Impacto:** una capacidad puede estar apagada y aun así existir rutas que la
ignoren. Para agnosticismo serio, el gating debe ser una decisión de perímetro
(gateway o middleware común por servicio), no una suma de guards dispersos.

### 4.3 El manifiesto de features es por empresa, no por negocio

`manifiesto` lee `identity.empresa_capacidades`
(`packages/identity-service/src/dominio/capacidades.rs:162-171`), mientras que
la autoridad por negocio es `identity.negocio_capacidades`
(`infra/db/init/78_negocios_capacidades.sql`). Los cambios de empresa se
replican a los negocios, pero un negocio con configuración propia no tiene un
manifiesto propio. Para cadenas con sucursales de distinto rubro, esto es una
limitación real.

### 4.4 La moneda y el redondeo fiscal no son multi-moneda

Además del `C$` fijo en comprobantes, el IVA se calcula sobre el total de la
orden, no sobre bases gravables por línea. Dos líneas con distinta tasa o
exención no se pueden representar.

## 5. Qué significa para el MVP

| Tipo de negocio | ¿Funciona hoy? | Bloqueador principal |
|---|---|---|
| Farmacia / retail de una empresa | **Sí, con fricción** | Debe crear tienda y asignar vendedor; comisión no configurable a 0; promociones no aplican. |
| Clínica / servicios agendados | **Sí, con fricción** | Mismo acoplamiento marketplace; la agenda y los recursos ya existen. |
| Distribuidora con varias bodegas | **No del todo** | El checkout no elige ubicación ni despacha desde una bodega concreta. |
| Empresa con dos países | **No** | Jurisdicción activa global, moneda fija y sin impuestos por línea. |
| Retail serio (cupones, RMA parcial, reseñas) | **No** | Faltan capacidades completas, no configuración. |
| Marketplace real multi-vendedor | **Parcial** | Hay comisión y tienda, pero no hay cargo/reembolso real ni conciliación. |

## 6. Plan de remediación propuesto

Ordenado por **impacto en MVP / riesgo / costo**. No todo debe entrar en el MVP.

### P0 — Quick wins (1–2 PRs, bajo riesgo)

1. **Comisión 0 soportada y respuesta consistente**
   - Permitir `tasa == 0` en `money::comision` de orders y commissions.
   - Aceptar `COMMISSION_RATE=0` en `config.rs`.
   - Usar `self.tasa_comision` también en la vista HTTP
     (`checkout.rs:521-524`).
   - Tests: tasa 0 → comisión 0; tasa 0.10 → respuesta == evento.
   - Esto no elimina tienda/vendedor, pero permite venta directa sin comisión.

2. **Manifiesto de features por negocio**
   - Añadir `GET /api/v2/empresas/{id}/negocios/{negocio_id}/features` o un
     query `?negocio_id=`.
   - Resolver capacidades desde `negocio_capacidades` con fallback a empresa.
   - Test de contrato y de integración con un negocio que difiere de la empresa.

3. **Corregir la documentación de estado**
   - Actualizar `docs/audits/2026-11-08-revision-integral.md` o dejar este
     documento como fuente vigente; el README no debe seguir prometiendo
     “gating uniforme”.

### P1 — Desbloqueos de modelo de negocio (1–2 semanas)

4. **Modo venta directa (sin marketplace)**
   - Introducir un flag efectivo por empresa/negocio, derivado del pack o de
     `configuracion_operativa`: `requiere_vendedor` / `requiere_tienda` /
     `comision`.
   - Hacer nullable `vendedor_id`/`tienda_id` en proyección y contrato, o
     poblar un vendedor “sistema” por empresa cuando el modo sea directo.
   - Saltar la acreditación de comisión cuando no hay vendedor.
   - Migración aditiva + ADR (R-DO-5) + OpenAPI + tests de contrato.
   - **Decisión de producto:** ¿la venta directa usa una tienda interna o no
     usa tienda? Recomendación: tienda interna automática para no romper el
     storefront.

5. **Ubicación de despacho**
   - Añadir `ubicacion_id` opcional a la línea de orden y al comando de
     checkout; si no viene, resolver por regla configurable (predeterminada /
     más cercana / con stock).
   - Inventory reserva y descuenta en esa ubicación; si no alcanza, error
     explícito, no reparto silencioso.
   - Tests de integración con dos bodegas y una orden que solo cabe en una.

6. **Promociones aplicadas en pricing**
   - Exponer un cálculo de promoción en `pricing-service` y consumirlo desde
     checkout (o desde stores al publicar).
   - Empezar por `descuento_porcentaje` y `2x1`, que son los de mayor uso.
   - Devolver el descuento desglosado por línea para que la orden sea auditable.

### P2 — Fiscal y pagos (mayor esfuerzo, fuera del MVP mínimo)

7. **Jurisdicción por tenant**
   - `finance.jurisdicciones` + configuración por empresa/negocio; eliminar la
     selección global por `ORDER BY codigo_pais`.
   - Moneda del comprobante desde la serie/jurisdicción, no `'C$'` literal.
   - Impuestos por línea con tasa/exención propias.

8. **Puerto de pagos real**
   - Interfaz `ProveedorPagos` (crear cargo, consultar, reembolsar, webhook)
     con adaptador simulado y adaptador real.
   - Reembolso total/parcial y conciliación; no mezclar con la lógica de orden.

### P3 — Retail completo (no MVP)

9. Cupones, reseñas, wishlist, RMA parcial, notas de crédito automáticas.
   Cada una es una capacidad nueva y debe entrar por el proceso de capacidad
   (migración + features + guard + endpoints + eventos), no como parche.

## 7. Qué no haría

- **No** convertir todo el dominio en un motor de reglas configurable. El costo
  de complejidad supera el beneficio para el MVP.
- **No** hacer nullable el vendedor sin una decisión de producto sobre la
  comisión y la autorización de lectura por vendedor.
- **No** aplicar promociones solo en el frontend: el precio real debe salir del
  backend y quedar en el evento.
- **No** añadir una segunda jurisdicción sin resolver antes la selección por
  tenant; hoy sería un bug silencioso.

## 8. Evidencia reproducible

```bash
# Órdenes exigen vendedor/tienda
grep -n "vendedor_id\|tienda_id" infra/db/init/01d_es_orders.sql | head
grep -n "pub vendedor_id\|pub tienda_id" packages/orders-service/src/clients.rs \
  packages/orders-service/src/order_state.rs packages/orders-service/src/views.rs

# Comisión hardcodeada en la respuesta
grep -n "0.12\|tasa_comision" packages/orders-service/src/checkout.rs

# Features cerradas
grep -n "FEATURES_POR_CAPACIDAD\|features_de" \
  packages/identity-service/src/dominio/features.rs

# Promociones solo CRUD
grep -rn "promocion" packages/pricing-service/src --include=*.rs
grep -rn "promocion" packages/orders-service/src packages/stores-service/src --include=*.rs

# Stock por ubicación
grep -n "predeterminada\|stock DESC" packages/inventory-service/src/stock.rs
grep -n "SUM(s.stock)" packages/inventory-service/src/reservas.rs

# Fiscal
grep -n "Ley 822\|'C\$'\|tasa_iva" infra/db/init/01_esquemas.sql | head
grep -n "WHERE activa ORDER BY codigo_pais" \
  packages/finance-service/src/tributacion.rs
grep -n "moneda.*'C\$'" packages/finance-service/src/facturacion.rs

# Pagos
grep -n "route(" packages/orders-service/src/http/pagos.rs
```

## 9. Conclusión

La información inicial **no es falsa**, pero sí es **demasiado optimista en la
tabla** y **demasiado plana en los matices**. Lo que hoy existe es una base
multi-tenant y multi-vertical muy sólida, con un flujo comercial de marketplace
difícil de desactivar y un motor fiscal de una sola jurisdicción.

Para el MVP, el orden correcto es:

1. **P0:** comisión 0 + respuesta consistente + manifiesto por negocio.
2. **P1:** modo venta directa, ubicación de despacho y promociones aplicadas.
3. **P2/P3:** fiscal multi-jurisdicción y pagos reales, cuando el producto lo
   exija.

Ese orden suma puntos reales al MVP sin reescribir la plataforma ni romper el
marketplace que ya funciona.

## 10. Seguimiento de implementación

- **P0.1 — Comisión 0 y respuesta consistente: implementado en esta rama**
  (`feat/agnosticismo-auditoria`). `money::comision` acepta `tasa = 0` en
  orders, commissions y finance; `COMMISSION_RATE=0` es válido en orders y
  commissions; la vista HTTP de la orden usa `self.tasa_comision` en lugar del
  `0.12` hardcodeado. Cubierto con tests unitarios en los tres crates.
- **P0.2 — Manifiesto de features por negocio:** pendiente.
- **P0.3 — Documentación de estado:** este documento queda como fuente vigente
  para el diagnóstico de agnosticismo.
- **P1 — Modo venta directa, ubicación de despacho y promociones:** pendiente
  de decisión de producto y ADR.

