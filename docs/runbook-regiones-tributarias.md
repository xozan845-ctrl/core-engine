# Runbook · Regiones tributarias

Operación del catálogo de regiones y de la selección por tenant (ADR-0049).

## Modelo en una línea

`finance.jurisdicciones` (reglas) → `identity.pack_regiones` (qué admite el
pack) → `identity.empresas.region_tributaria` (qué eligió el dueño) →
`finance.configuracion_fiscal` / series (cómo factura).

## Verificar el estado

```sql
-- Regiones activas y su descriptor
SELECT codigo_pais, nombre, moneda, moneda_iso, tasa_iva, locale,
       zona_horaria, activa
  FROM finance.jurisdicciones
 ORDER BY codigo_pais;

-- Regiones por pack (debe haber exactamente una predeterminada por pack)
SELECT pack_codigo, pack_version, region_codigo, predeterminada, activo
  FROM identity.pack_regiones
 ORDER BY pack_codigo, pack_version, predeterminada DESC, region_codigo;

-- Tenants sin región (no debería haber en instalaciones nuevas)
SELECT id, nombre, pack_codigo, pack_version, region_tributaria
  FROM identity.empresas
 WHERE region_tributaria IS NULL;

-- Historial de cambios
SELECT empresa_id, region_anterior, region_nueva, motivo, creado_en
  FROM identity.empresa_region_historial
 ORDER BY creado_en DESC
 LIMIT 50;
```

Chequeo automatizado:

```bash
npm run multi-vertical:check
```

Falla si una empresa no tiene región, si su región no está declarada por su
pack, si un pack activo no declara regiones o si no hay exactamente una
predeterminada. El `PUT /packs/{codigo}/{version}/regiones` rechaza retirar una
región que ya usan empresas (`REGION_EN_USO`).

## Añadir una región (país o subdivisión)

1. Insertar la fila fiscal. Ejemplo de estructura; los valores deben provenir
   de la ley vigente, no de una suposición:

```sql
INSERT INTO finance.jurisdicciones
  (codigo_pais, nombre, pais_iso, moneda, moneda_iso, simbolo_moneda,
   tasa_iva, tasa_ir, periodicidad_declaracion, leyes, activa,
   locale, zona_horaria, documento_fiscal, perfil, vigente_desde)
VALUES
  ('CR', 'Costa Rica', 'CR', '₡', 'CRC', '₡', 0.1300, 0.3000, 'MENSUAL',
   '{}'::jsonb, false, 'es-CR', 'America/Costa_Rica',
   '{"tipo":"CÉDULA","emisor":"HACIENDA"}'::jsonb,
   '{"features":["fiscal.iva","fiscal.declaraciones"]}'::jsonb,
   CURRENT_DATE);
```

Dejar `activa = false` hasta validar tasas, documento y calendario con
contabilidad/legal.

2. Declararla en los packs que la soporten:

```sql
INSERT INTO identity.pack_regiones
  (pack_codigo, pack_version, region_codigo, predeterminada, activo)
VALUES ('pack_retail', 1, 'CR', false, true)
ON CONFLICT (pack_codigo, pack_version, region_codigo)
DO UPDATE SET activo = EXCLUDED.activo, actualizado_en = NOW();
```

3. Configurar retención y presentación con `PATCH
   /finanzas/jurisdicciones/{codigo}`:

```bash
curl -X PATCH http://localhost:8080/api/v2/finanzas/jurisdicciones/CR \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"retencion_activa":true,"tasa_retencion_por_mil":20,
       "retencion_codigo":"IR","locale":"es-CR",
       "zona_horaria":"America/Costa_Rica"}'
```

4. Configurar su catálogo de tasas (mínimo `GENERAL` y `EXENTA`):

```bash
curl -X PUT http://localhost:8080/api/v2/finanzas/jurisdicciones/CR/tasas \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"tasas":[
    {"codigo":"GENERAL","nombre":"Tasa general","tasa_por_mil":130},
    {"codigo":"EXENTA","nombre":"Exento","tasa_por_mil":0,"exenta":true}
  ]}'
```

5. Activarla cuando esté validada:

```sql
UPDATE finance.jurisdicciones SET activa = true WHERE codigo_pais = 'CR';
```

6. Crear las series fiscales por empresa/región:

```bash
curl -X POST http://localhost:8080/api/v2/finanzas/series \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"serie":"CR001","tipo":"FACTURA","prefijo":"CR001-",
       "ruc_emisor":"RUC-EMPRESA","jurisdiccion":"CR",
       "empresa_id":"<empresaId>"}'
```

7. Si aplica facturación electrónica, configurar
   `proveedor_electronico=dgi` (o `http`) y el endpoint/credenciales
   (`FACTURACION_PROVEEDOR_URL`, `FACTURACION_PROVEEDOR_KEY`).
   Sin certificación del proveedor, `EMITIR_COMPROBANTES_FISCALES` debe quedar
   en `false`; si el piloto opera sin comprobante fiscal válido, dejar
   `FACTURACION_PERMITIR_SIN_CERTIFICADO=true` y asumir que el PDF saldrá
   marcado **NO FISCAL**. Ver
   [`pendientes-facturacion-electronica.md`](./pendientes-facturacion-electronica.md).

8. Probar el selector:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/api/v2/regiones-tributarias?pack_codigo=pack_retail&pack_version=1"
```

## Errores frecuentes

| Código | Causa | Acción |
|---|---|---|
| `REGION_NO_DISPONIBLE` | El pack no declara la región o no hay regiones activas | Revisar `identity.pack_regiones` y `finance.jurisdicciones.activa` |
| `REGION_NO_ASIGNADA` | Empresa legacy sin región | `PUT /empresas/{id}/region-tributaria` o backfill |
| `REGION_PREDETERMINADA_UNICA` | Un pack tiene más de una predeterminada | Dejar una sola con `predeterminada = true` |
| `REGION_EN_USO` | Se intentó retirar del pack una región que ya usan sus empresas | Cambiar la región de esas empresas o mantenerla declarada |
| `SIN_JURISDICCION` | Una serie apunta a una región inexistente | Corregir `series_comprobantes.jurisdiccion` o crear la región |
| `PACK_VERSION_REQUERIDA` | Se consultó el selector con `pack_codigo` y sin versión | Enviar `pack_version` |

## Cambiar la región de un tenant

```bash
curl -X PUT http://localhost:8080/api/v2/empresas/<empresaId>/region-tributaria \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"region_tributaria":"NI","motivo":"Cambio de domicilio fiscal"}'
```

Efectos:

- Actualiza `pais`, `moneda` y `zona_horaria` de la empresa.
- Realinea `finance.configuracion_fiscal` existente.
- Deja rastro en `identity.empresa_region_historial` y `identity.auditoria`.
- **No** reescribe comprobantes ni asientos ya emitidos.

## Rollback

La migración es aditiva. Para revertir la selección sin borrar el historial:

```sql
-- Volver a la región anterior de una empresa concreta
UPDATE identity.empresas SET region_tributaria = 'NI' WHERE id = '<uuid>';
```

No eliminar `finance.jurisdicciones` referenciadas por empresas, series o
comprobantes: la FK lo impide y hacerlo rompería la trazabilidad fiscal.

## Seguridad y RLS

- `identity.pack_regiones` es catálogo de plataforma: lectura para cualquier
  autenticado con pack activo; escritura solo `super_admin`/`admin`.
- `identity.empresa_region_historial` tiene política por `empresa_id`; un
  empresario solo ve el historial de su tenant.
- `finance.jurisdicciones` lo administra finance; identity solo lo lee para
  resolver la región y el manifiesto.

## Cierre fiscal

- **Tasas por región:** `GET/PUT /api/v2/finanzas/jurisdicciones/{codigo}/tasas`.
  Debe existir `GENERAL` activa; `EXENTA` se usa para líneas sin IVA.
- **Exención por línea:** el ítem expone `atributos.impuesto_codigo` o la línea
  envía `impuesto_codigo`. Un código desconocido falla con
  `IMPUESTO_NO_DISPONIBLE`.
- **Retención:** se configura en la región (`retencion_activa`,
  `tasa_retencion`, `retencion_codigo`). El comprobante guarda el monto y el
  PDF lo desglosa.
- **Series:** `GET/POST /api/v2/finanzas/series` y
  `PATCH /api/v2/finanzas/series/{serie}`. La emisión prefiere la serie del
  tenant y su región.
- **Declaraciones:** el planificador genera la de plataforma y una por tenant
  con comprobantes. También se puede forzar con
  `POST /api/v2/finanzas/declaraciones/generar` y `empresa_id` (admin).
- **Moneda:** notificaciones resuelve el símbolo por ISO/símbolo y deja el
  código visible si no lo conoce.
- **DGI:** `proveedor_electronico=dgi` usa el adaptador XML de referencia
  contra el gateway configurado. La certificación y credenciales son
  operativas del proveedor autorizado.

## Verificación de cierre

```bash
npm run rules:check
npm run multi-vertical:check
```

Los pendientes fiscales del bloque de regiones quedaron cerrados en ADR-0049 y
en [`docs/backlog-plataforma.md`](./backlog-plataforma.md).

## Documentación relacionada

- [ADR-0049](./adr/0049-regiones-tributarias.md) — decisión y criterios.
- [ADR-0026](./adr/0026-fiscalidad-por-tenant.md) — configuración fiscal por
  tenant y override.
- [ADR-0031](./adr/0031-facturacion-electronica-pdf.md) — facturación
  electrónica y PDF.
- [ADR-0038](./adr/0038-iva-por-linea-checkout.md) — IVA por línea.
- [ADR-0047](./adr/0047-onboarding-idempotente.md) — onboarding que recibe la
  región.
