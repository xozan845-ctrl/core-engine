# ADR-0019: Fidelización como capacidad seleccionable por configuración

- **Fecha**: 2026-11-16
- **Estado**: Aceptado — implementación en curso (fases 1-3: capacidad, alta de
  empresa, modelo de datos, administración por empresa, acumulación al pagar y
  vista del comprador)
- **Ámbito**: `identity-service` (capacidades, verticales, packs, alta de
  empresa, manifiesto), `orders-service` (programas, tarjetas, movimientos,
  recompensas y canjes), PostgreSQL/RLS, OpenAPI, CI/CD y documentación
- **Referencias**: [ADR-0001](./0001-plataforma-multi-vertical.md),
  [ADR-0002](./0002-envelope-y-desacoplamiento.md),
  [ADR-0013](./0013-orders-service-en-rust.md),
  [ADR-0017](./0017-identity-service-en-rust.md)

## Contexto

La plataforma crea negocios por **vertical + pack + capacidades**. Un pack
declara qué capacidades quedan activas y el manifiesto
(`GET /api/v2/empresas/:id/features`) le dice al frontend qué módulos mostrar.
Hoy existen 22 capacidades; ninguna cubre tarjetas de fidelización, puntos,
recompensas ni canjes.

Los usuarios piden ofrecer fidelización como **add-on opcional al crear su
negocio**. La pregunta de diseño es si eso se resuelve con configuración pura o
si exige una capacidad nueva.

La respuesta es que **no es configuración pura**:

- No existe la capacidad `fidelizacion`, por lo que ningún pack puede
  declararla.
- No existe el modelo de datos (programa, tarjeta, movimientos, recompensas,
  canjes) ni las reglas de acumulación, expiración y canje.
- No existe integración con el ciclo de vida de la orden (`pagada`).

Sin embargo, una vez implementada **sí debe comportarse como configuración**:
seleccionable por vertical, pack o add-on, y visible solo cuando está activa.
Ese es el patrón que ya siguen pagos, agenda y fulfillments.

## Decisión

### 1. Fidelización es una capacidad nueva, no un pack

Se registra la capacidad `fidelizacion` (módulo `loyalty`) en
`identity.capacidades` y se ofrece como **add-on inactivo por defecto** en todas
las verticales activas y en todos los packs activos. Estar presente pero
inactiva permite mostrarla como opción sin activarla sin consentimiento.

El manifiesto expone:

| Capacidad | Features |
|---|---|
| `fidelizacion` | `fidelizacion.programas`, `fidelizacion.tarjetas`, `fidelizacion.canjes` |

### 2. Selección en el alta de empresa

`POST /api/v2/empresas` acepta `capacidades_extra: ["fidelizacion"]`. El alta:

1. valida que la capacidad exista y esté disponible en la vertical elegida;
2. activa la capacidad en `identity.empresa_capacidades` dentro de la misma
   transacción que crea la empresa;
3. responde `400 CAPACIDAD_NO_DISPONIBLE` si la capacidad no está permitida.

La activación posterior sigue disponible por
`PATCH /api/v2/empresas/:id/capacidades/fidelizacion`, que ya existía.

Se descarta crear un pack por combinación (`pack_retail_fidelidad`, …): multiplica
el catálogo y no aporta nada frente al add-on explícito.

### 3. Ubicación de la implementación (MVP)

La primera implementación vive en **`orders-service`**, porque es el dueño del
ciclo de vida de la orden y de los eventos de pago:

| Tabla | Contenido |
|---|---|
| `orders.fidelizacion_programas` | Programa por empresa: activo, puntos por moneda, expiración y reglas |
| `orders.fidelizacion_tarjetas` | Tarjeta por empresa + parte (cliente), con código y saldo |
| `orders.fidelizacion_movimientos` | Acumulaciones, canjes, ajustes y expiraciones; `order_id`/`event_id` para idempotencia |
| `orders.fidelizacion_recompensas` | Catálogo de premios y costo en puntos |
| `orders.fidelizacion_canjes` | Canje de recompensa por tarjeta, con estado y vínculo a la orden |

Reglas:

- Los puntos se acumulan **cuando la orden pasa a `pagada`**, nunca al crearse.
- La acumulación es **idempotente por orden** (`UNIQUE (tarjeta_id, order_id,
  tipo)` en movimientos de acumulación).
- El canje valida saldo, descuenta puntos y registra el movimiento en la misma
  transacción.
- RLS por `empresa_id`; el cliente solo ve su tarjeta y sus movimientos.
- Los montos siguen la regla global: enteros en centavos y strings de dos
  decimales en JSON.

La fase 2 implementa el modelo, el dominio (`programa`, `tarjetas`,
`movimientos`, `recompensas`, `canjes`), los endpoints de administración por
empresa y la acumulación idempotente como método de dominio. La fase 3 conecta
la acumulación con `Checkout::transicionar` (pago automático, confirmación
manual y webhook) y expone `mis-tarjetas` al comprador, derivando la propiedad
de la tarjeta de sus propias órdenes (`orden_vista.comprador_id`) para no
exponer tarjetas de otros clientes del mismo tenant.

### 4. Servicio propio como evolución

Si fidelización crece a campañas, segmentos, niveles múltiples o marketing
automatizado, se extrae a `loyalty-service` como bounded context propio (mismo
patrón que `commissions` o `finance`). La capacidad y sus features no cambian:
`orders` pasa de dueño a consumidor por API interna y eventos.

## Consecuencias

- **Positivas**:
  - El add-on se elige en el asistente de alta sin desplegar código.
  - El frontend no hardcodea fidelización: depende del manifiesto.
  - La migración es aditiva y no rompe empresas existentes.
  - La idempotencia evita puntos duplicados por reintentos o webhooks.
- **Negativas / costes**:
  - `orders-service` asume un dominio más; el ADR fija el criterio de extracción
    futura.
  - Requiere migración, RLS y contrato OpenAPI nuevos.
  - Los packs creados después del despliegue no incluyen fidelización
    automáticamente; el admin debe declararla o usar `capacidades_extra`.
- **Neutras**:
  - La capacidad queda inactiva por defecto: sin acción del usuario no cambia
    ninguna experiencia existente.

## Criterios de aceptación

1. `identity.capacidades` contiene `fidelizacion` con módulo `loyalty`.
2. La vertical `retail` (y las demás activas) permiten activarla; una capacidad
   inexistente o no disponible devuelve `400 CAPACIDAD_NO_DISPONIBLE`.
3. `POST /api/v2/empresas` con `capacidades_extra: ["fidelizacion"]` crea la
   empresa con la capacidad activa.
4. `GET /api/v2/empresas/:id/features` incluye `fidelizacion.programas`,
   `fidelizacion.tarjetas` y `fidelizacion.canjes` solo si está activa.
5. Las tablas de fidelización tienen RLS por empresa y pruebas de aislamiento.
6. La acumulación por orden es idempotente y el canje es transaccional.
7. `scripts/config/db-migrations.json` registra las migraciones nuevas y los
   gates (`rules`, `lint`, `env`, `swagger`, `clippy`, cobertura) pasan.
