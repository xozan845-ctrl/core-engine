-- 99zz_domain_functions.sql — API de dominio transitoria (ADR-0051)
--
-- Estas funciones viven en el esquema **dueño** del dato y permiten que otro
-- servicio solicite una transición sin escribir directamente en sus tablas
-- (R-DB-1/R-MS-16). Son una API interna transitoria: se ejecutan en la misma
-- transacción del llamador para conservar atomicidad, con permisos acotados y
-- sin exponer SQL de tabla al servicio llamador.
--
-- Fase 2 las sustituirá por eventos/HTTP cuando el flujo de saga lo permita.

-- ── item: series serializadas ───────────────────────────────────────────────

CREATE OR REPLACE FUNCTION item.marcar_series_vendidas(
  p_order_id uuid,
  p_series_ids uuid[],
  p_garantia_meses integer
) RETURNS integer
LANGUAGE plpgsql
AS $$
DECLARE
  actualizadas integer;
BEGIN
  UPDATE item.item_series
     SET estado = 'vendido',
         order_id = p_order_id,
         vendido_en = NOW(),
         ubicacion_id = NULL,
         garantia_hasta = CASE
           WHEN p_garantia_meses IS NULL THEN NULL
           ELSE (CURRENT_DATE + (p_garantia_meses || ' months')::interval)::date
         END,
         actualizado_en = NOW()
   WHERE id = ANY(p_series_ids)
     AND estado = 'disponible';
  GET DIAGNOSTICS actualizadas = ROW_COUNT;
  RETURN actualizadas;
END;
$$;

CREATE OR REPLACE FUNCTION item.liberar_series(
  p_order_id uuid,
  p_series_ids uuid[],
  p_ubicacion_id uuid
) RETURNS integer
LANGUAGE plpgsql
AS $$
DECLARE
  actualizadas integer;
BEGIN
  UPDATE item.item_series s
     SET estado = 'disponible',
         order_id = NULL,
         vendido_en = NULL,
         ubicacion_id = COALESCE(p_ubicacion_id, s.ubicacion_id),
         actualizado_en = NOW()
   WHERE s.id = ANY(p_series_ids)
     AND s.order_id = p_order_id
     AND s.estado = 'vendido';
  GET DIAGNOSTICS actualizadas = ROW_COUNT;
  RETURN actualizadas;
END;
$$;

CREATE OR REPLACE FUNCTION item.devolver_series(
  p_order_id uuid,
  p_series_ids uuid[],
  p_ubicacion_id uuid
) RETURNS integer
LANGUAGE plpgsql
AS $$
DECLARE
  actualizadas integer;
BEGIN
  UPDATE item.item_series s
     SET estado = 'devuelto',
         devuelto_en = NOW(),
         ubicacion_id = COALESCE(p_ubicacion_id, s.ubicacion_id),
         actualizado_en = NOW()
   WHERE s.id = ANY(p_series_ids)
     AND s.order_id = p_order_id
     AND s.estado = 'vendido';
  GET DIAGNOSTICS actualizadas = ROW_COUNT;
  RETURN actualizadas;
END;
$$;

CREATE OR REPLACE FUNCTION item.marcar_tipo_precio(
  p_empresa_id uuid,
  p_item_id uuid,
  p_tipo text
) RETURNS integer
LANGUAGE plpgsql
AS $$
DECLARE
  actualizados integer;
BEGIN
  IF p_tipo NOT IN ('fijo', 'escala') THEN
    RAISE EXCEPTION 'tipo_precio invalido: %', p_tipo USING ERRCODE = '22023';
  END IF;
  UPDATE item.items
     SET tipo_precio = p_tipo,
         actualizado_en = NOW()
   WHERE id = p_item_id
     AND empresa_id = p_empresa_id
     AND tipo_precio <> p_tipo;
  GET DIAGNOSTICS actualizados = ROW_COUNT;
  RETURN actualizados;
END;
$$;

-- ── inventory: ubicación predeterminada ─────────────────────────────────────

CREATE OR REPLACE FUNCTION inventory.crear_ubicacion_predeterminada(
  p_empresa_id uuid,
  p_negocio_id uuid
) RETURNS integer
LANGUAGE plpgsql
AS $$
DECLARE
  creadas integer;
BEGIN
  INSERT INTO inventory.ubicaciones
    (empresa_id, negocio_id, codigo, nombre, tipo, predeterminada)
  VALUES (
    p_empresa_id,
    p_negocio_id,
    'principal_' || substr(replace(p_negocio_id::text, '-', ''), 1, 8),
    'Principal',
    'bodega',
    true
  )
  ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS creadas = ROW_COUNT;
  RETURN creadas;
END;
$$;

-- ── finance: configuración fiscal de empresa ────────────────────────────────

CREATE OR REPLACE FUNCTION finance.actualizar_region_fiscal(
  p_empresa_id uuid,
  p_jurisdiccion text
) RETURNS integer
LANGUAGE plpgsql
AS $$
DECLARE
  actualizadas integer;
BEGIN
  UPDATE finance.configuracion_fiscal
     SET jurisdiccion = p_jurisdiccion,
         actualizado_en = NOW()
   WHERE empresa_id = p_empresa_id
     AND negocio_id IS NULL;
  GET DIAGNOSTICS actualizadas = ROW_COUNT;
  RETURN actualizadas;
END;
$$;

-- ── permisos ────────────────────────────────────────────────────────────────

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT EXECUTE ON FUNCTION item.marcar_series_vendidas(uuid, uuid[], integer) TO core_engine_app;
    GRANT EXECUTE ON FUNCTION item.liberar_series(uuid, uuid[], uuid) TO core_engine_app;
    GRANT EXECUTE ON FUNCTION item.devolver_series(uuid, uuid[], uuid) TO core_engine_app;
    GRANT EXECUTE ON FUNCTION item.marcar_tipo_precio(uuid, uuid, text) TO core_engine_app;
    GRANT EXECUTE ON FUNCTION inventory.crear_ubicacion_predeterminada(uuid, uuid) TO core_engine_app;
    GRANT EXECUTE ON FUNCTION finance.actualizar_region_fiscal(uuid, text) TO core_engine_app;
  END IF;
END;
$$;

COMMENT ON FUNCTION item.marcar_series_vendidas(uuid, uuid[], integer) IS
  'API transitoria ADR-0051: inventory marca series vendidas en la misma transacción.';
COMMENT ON FUNCTION item.liberar_series(uuid, uuid[], uuid) IS
  'API transitoria ADR-0051: libera series de una orden cancelada.';
COMMENT ON FUNCTION item.devolver_series(uuid, uuid[], uuid) IS
  'API transitoria ADR-0051: marca series devueltas de una RMA.';
COMMENT ON FUNCTION item.marcar_tipo_precio(uuid, uuid, text) IS
  'API transitoria ADR-0051: pricing mantiene el flag tipo_precio del item.';
COMMENT ON FUNCTION inventory.crear_ubicacion_predeterminada(uuid, uuid) IS
  'API transitoria ADR-0051: identity crea la ubicación principal al dar de alta empresa/negocio.';
COMMENT ON FUNCTION finance.actualizar_region_fiscal(uuid, text) IS
  'API transitoria ADR-0051: identity sincroniza la región fiscal de la empresa.';
