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

Falla si una empresa no tiene región, si un pack activo no declara regiones o
si no hay exactamente una predeterminada.

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

3. Activarla cuando esté validada:

```sql
UPDATE finance.jurisdicciones SET activa = true WHERE codigo_pais = 'CR';
```

4. Crear las series fiscales de esa región con `jurisdiccion = 'CR'`
   (la columna ya no tiene default) y configurar el proveedor electrónico por
   tenant si aplica.

5. Probar el selector:

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
