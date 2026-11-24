# ADR-0049 — Regiones tributarias seleccionables por pack y tenant

- **Fecha:** 2026-11-23
- **Estado:** Aceptado (implementado; piloto Nicaragua)
- **Relacionado:** [ADR-0026](./0026-fiscalidad-por-tenant.md),
  [ADR-0031](./0031-facturacion-electronica-pdf.md),
  [ADR-0038](./0038-iva-por-linea-checkout.md),
  [ADR-0001](./0001-plataforma-multi-vertical.md)

## Contexto

La plataforma ya separaba vertical (qué negocio) de pack (qué configuración) y
capacidad (qué autoridad de negocio). La fiscalidad estaba resuelta a medias:

- `finance.jurisdicciones` era un catálogo multijurisdicción, pero no había una
  selección explícita del dueño del negocio al crear la empresa.
- `finance.configuracion_fiscal` permitía fijar la jurisdicción por empresa o
  negocio, pero solo después del alta y vía una ruta de finanzas.
- `identity.empresas` nacía con `pais`, `moneda` y `zona_horaria` hardcodeados a
  Nicaragua/NIO/America/Managua.
- Las series de comprobantes asumían `jurisdiccion = 'NI'` por defecto.
- El manifiesto de features no exponía la región ni las capacidades fiscales
  que esa región habilita.

Resultado: el país estaba repartido entre defaults, seeds y código. Añadir un
segundo país exigía tocar varios servicios y no había forma de que el
empresario eligiera su régimen fiscal al aplicar un pack.

## Decisión

1. **La región tributaria es una dimensión de configuración, no una capacidad
   booleana.** Una capacidad enciende o apaga una autoridad (`facturacion`);
   una región elige un conjunto de reglas (moneda, IVA, documento fiscal,
   periodicidad, features). Modelarla como `capacidad + config` habría
   escondido la selección en un JSON sin catálogo ni validación.

2. **Catálogo canónico en `finance.jurisdicciones`.** Se enriquece con
   `pais_iso`, `subdivision`, `moneda_iso`, `locale`, `zona_horaria`,
   `documento_fiscal`, `perfil` y `vigente_desde`. Las reglas fiscales siguen
   siendo propiedad de finance; identity solo lee el catálogo.

3. **El pack declara sus regiones.** Nueva tabla `identity.pack_regiones`
   (`pack_codigo`, `pack_version`, `region_codigo`, `predeterminada`,
   `activo`). Un pack sin regiones activas no puede crear empresas
   (`REGION_NO_DISPONIBLE`). Si el admin crea un pack sin declararlas, se
   asignan las regiones activas y la primera queda predeterminada.

4. **La empresa elige la región al aplicar el pack.** Nueva columna
   `identity.empresas.region_tributaria` (FK a la jurisdicción) y
   `region_tributaria_desde`. El alta (`POST /empresas` y `POST /onboarding`)
   acepta `region_tributaria`. Si se omite, se usa la predeterminada del pack.

5. **El perfil hereda la región, no un país fijo.** `pais`, `moneda` y
   `zona_horaria` se resuelven desde la región elegida cuando el cliente no los
   envía. Desaparecen los defaults `Nicaragua`/`NIO`/`America/Managua` del
   código de alta.

6. **Finance resuelve la jurisdicción efectiva en este orden:**
   negocio (`finance.configuracion_fiscal` con `negocio_id`) → empresa
   (`configuracion_fiscal` con `negocio_id NULL`) → región del tenant
   (`identity.empresas.region_tributaria`) → primera jurisdicción activa
   (solo compatibilidad legacy).

7. **Cambio de región con historial.** `PUT
   /api/v2/empresas/{id}/region-tributaria` valida que la región pertenezca al
   pack, actualiza `pais`/`moneda`/`zona_horaria`, realinea el override fiscal
   de la empresa (respeta los overrides por negocio) y registra
   `identity.empresa_region_historial` + auditoría
   (`empresa.region_tributaria_cambiada`). No reescribe documentos ya emitidos:
   la región nueva aplica a operaciones futuras. Un cambio de pack exige que el
   pack destino admita la región actual (`REGION_NO_COMPATIBLE_CON_PACK`).

8. **Manifiesto con la región.** `GET /api/v2/empresas/{id}/features` incluye
   `region_tributaria` (código, moneda, locale, zona horaria, tasa de IVA,
   documento fiscal) y añade sus `perfil.features` fiscales a `features` solo
   si la capacidad `facturacion` está activa. El frontend compone el checkout y
   las pantallas fiscales sin hardcodear el país.

9. **Fail-closed en tramos fiscales.** La emisión de comprobantes ya no asume
   `15 %`/`NIO` si la serie apunta a una región inexistente: responde
   `SIN_JURISDICCION`. La columna `series_comprobantes.jurisdiccion` deja de
   tener default `'NI'`; cada serie debe declarar su región.

10. **Cierre del bloque fiscal.** La región no solo elige país: también
    parametriza el detalle operativo:
    - **Catálogo de tasas** (`finance.tasas_impuesto`): `GENERAL`, `EXENTA` y
      las que agregue el admin. `finance.jurisdicciones.tasa_default_codigo`
      define la tasa por defecto.
    - **Exenciones y múltiples tasas por línea**: la orden resuelve
      `impuesto_codigo` (override de la línea o
      `atributos.impuesto_codigo` del ítem) contra el catálogo de la región y
      guarda el snapshot `tasa_impuesto_bps`/`impuesto_cents`.
    - **Retenciones**: `retencion_activa`, `tasa_retencion` y
      `retencion_codigo` por región; el comprobante guarda
      `retencion_cents`/`retencion_tasa_bps` y el PDF lo desglosa.
    - **Series por empresa/región**: `series_comprobantes.empresa_id` +
      Jurisdicción. La emisión prefiere serie del tenant y región, luego
      global. API de alta/listado/estado.
    - **Declaraciones por tenant**: `declaraciones.empresa_id`; el IVA sale de
      los comprobantes EMITIDOS del tenant y la declaración de plataforma
      sigue usando el libro mayor. El planificador genera ambos.
    - **Formato de moneda**: notificaciones resuelve el símbolo por ISO/símbolo
      y deja el código visible si no lo conoce; ya no asume `C$`.
    - **Adaptador DGI de referencia**: `proveedor_electronico=dgi` genera XML
      fiscal y lo publica en el gateway configurado. La certificación,
      credenciales y endpoint los provee el país/proveedor autorizado.

## Contrato

- `GET /api/v2/regiones-tributarias?pack_codigo=&pack_version=`
- `GET /api/v2/empresas/{id}/region-tributaria`
- `PUT /api/v2/empresas/{id}/region-tributaria`
- `GET /api/v2/packs` y `GET /api/v2/packs/{codigo}/{version}` incluyen
  `regiones`.
- `POST /api/v2/empresas` y `POST /api/v2/onboarding` aceptan
  `region_tributaria`.
- `GET /api/v2/empresas/{id}/features` incluye `region_tributaria`.
- `GET /api/v2/finanzas/jurisdicciones` devuelve el descriptor ampliado.
- `PATCH /api/v2/finanzas/jurisdicciones/{codigo}` actualiza estado, tasas,
  retención y presentación de una región existente.
- `GET /api/v2/packs/{codigo}/{version}/regiones` y `PUT` del mismo path para
  que un admin ajuste las regiones del pack sin recrearlo.
- `GET/PUT /api/v2/finanzas/jurisdicciones/{codigo}/tasas` para el catálogo de
  tasas (incluye `EXENTA`).
- `GET/POST /api/v2/finanzas/series` y `PATCH /api/v2/finanzas/series/{serie}`
  para series por empresa/región.
- `POST /api/v2/finanzas/declaraciones/generar` acepta `empresa_id` (admin);
  el empresario siempre declara su propio tenant.
- `GET /api/v2/finanzas/comprobantes` expone `retencion_cents` y
  `retencion_tasa_bps`.

## Datos

Migraciones aditivas `99_regiones_tributarias.sql` y
`99z_regiones_fiscal_completo.sql`:

- `finance.jurisdicciones`: descriptor de región, `tasa_default_codigo` y
  retenciones.
- `finance.tasas_impuesto`: catálogo de tasas por región con semilla
  `GENERAL`/`EXENTA`.
- `identity.pack_regiones`: catálogo pack ↔ región, con una predeterminada.
- `identity.empresas.region_tributaria` (+ fecha) con backfill a `NI`.
- `identity.empresa_region_historial`.
- `finance.series_comprobantes.empresa_id` + índice y selección por región.
- `finance.comprobantes.retencion_cents`/`retencion_tasa_bps`.
- `finance.declaraciones.empresa_id` + unicidad por tenant/periodo/tipo.
- Grants y RLS para tasas, declaraciones y series por empresa.

## Cómo se añade un país

1. `INSERT` en `finance.jurisdicciones` con su moneda ISO, tasas, documento
   fiscal, locale, zona horaria y `perfil.features`.
2. Declararlo en `identity.pack_regiones` para los packs que lo soporten (o
   crear una nueva versión del pack).
3. Configurar su catálogo de tasas con
   `PUT /finanzas/jurisdicciones/{codigo}/tasas` (mínimo `GENERAL` y, si
   aplica, `EXENTA`).
4. Si aplica facturación electrónica, elegir `proveedor_electronico=dgi` o
   `http` y configurar el gateway/credenciales (ADR-0031). El adaptador DGI de
   referencia ya genera el XML; la certificación es operativa.
5. Crear las series fiscales del tenant con `jurisdiccion` explícita.
6. No se modifica código de dominio ni se añade un servicio por país.

## Consecuencias

- **Positivas:** el país es configuración; el dueño elige al aplicar el pack;
  el manifiesto comunica la fiscalidad efectiva; los defaults dejan de mentir;
  añadir regiones es un cambio de datos + adaptador, no un fork.
- **Compatibilidad:** las empresas existentes quedan en `NI`; los packs
  existentes heredan `NI` como predeterminada; las órdenes y comprobantes
  históricos no cambian.
- **Límite:** cambiar de región no recalcula documentos ya emitidos ni migra
  series históricas. La declaración de plataforma sigue usando el libro mayor;
  la del tenant usa comprobantes y los gastos del tenant
  (`workspace_registros`).
- **Operativo:** el adaptador DGI de referencia requiere endpoint, credenciales
  y certificación del proveedor autorizado; sin ellos el comprobante queda en
  `electronica_estado='error'` y puede reintentarse.
- **Sin pendientes de producto en este bloque:** series por empresa/región,
  catálogo de tasas, exenciones/múltiples tasas, retenciones, declaraciones
  por tenant, formato de moneda y CRUD de regiones de pack están implementados
  y cubiertos por pruebas.

## Criterios de aceptación

- Un pack sin regiones activas rechaza el alta con `REGION_NO_DISPONIBLE`.
- Una región no declarada por el pack rechaza el alta o el cambio con
  `REGION_NO_DISPONIBLE`.
- Sin `region_tributaria`, el alta usa la predeterminada del pack.
- El perfil de una empresa nueva toma país, moneda ISO y zona horaria de la
  región, no de un literal.
- `GET /features` expone `region_tributaria`; sus features fiscales aparecen
  solo con `facturacion` activa.
- Finance emite con la región del tenant cuando no hay override explícito.
- Cambiar región deja historial y auditoría.
- Un admin puede listar y reemplazar las regiones de un pack por API; no puede
  retirar una región que ya usan empresas de ese pack (`REGION_EN_USO`).
- El catálogo de tasas admite `GENERAL`, `EXENTA` y tasas adicionales; una
  línea con código desconocido falla con `IMPUESTO_NO_DISPONIBLE`.
- Una línea con `impuesto_codigo=EXENTA` no paga IVA y se reporta en
  `exento_cents`.
- Con retención activa, el comprobante guarda el monto y el PDF lo desglosa.
- La emisión prefiere una serie del tenant y su región; sin serie compatible
  falla con `SERIE_NO_DISPONIBLE`.
- Un tenant puede generar su declaración de IVA/IR desde sus comprobantes; el
  planificador también las genera por tenant.
- Notificaciones muestra el símbolo de la moneda o el código, sin asumir `C$`.
- `npm run multi-vertical:check` falla si una empresa o pack activo queda sin
  región.

## Documentación relacionada

- Runbook operativo:
  [`docs/runbook-regiones-tributarias.md`](../runbook-regiones-tributarias.md).
- Onboarding: [ADR-0047](./0047-onboarding-idempotente.md).
- Fiscalidad por tenant y override: [ADR-0026](./0026-fiscalidad-por-tenant.md).
- IVA por línea: [ADR-0038](./0038-iva-por-linea-checkout.md).
- Facturación electrónica: [ADR-0031](./0031-facturacion-electronica-pdf.md).
- Pendientes de escala: [`docs/backlog-plataforma.md`](../backlog-plataforma.md).

## Verificación

- `cargo test` en `identity-service`: 138 unitarios, 35 HTTP y 10 PostgreSQL
  desde cero con ambas migraciones de región y CRUD de regiones de pack.
- `cargo test` en `finance-service`: 93 unitarios y 18 PostgreSQL con fallback
  de región, `SIN_JURISDICCION`, tasas, retención, serie por tenant y
  declaración por tenant.
- `cargo test` en `orders-service`: 135 unitarios y 55 PostgreSQL, incluida
  exención por línea y `IMPUESTO_NO_DISPONIBLE`.
- `cargo test` en `notifications-service`: 72 unitarios, incluido el símbolo
  por moneda.
- `npm run rules:check`: 84/84 (agnosticismo, migraciones, RLS, OpenAPI).
- `npm run multi-vertical:check`: toda empresa y pack activo con región; una
  sola predeterminada por pack.
