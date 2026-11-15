-- Esquema catalog para pruebas de integración.
CREATE EXTENSION IF NOT EXISTS btree_gist;
DROP SCHEMA IF EXISTS catalog CASCADE;
--
-- PostgreSQL database dump
--

-- Dumped from database version 16.4
-- Dumped by pg_dump version 16.4

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: catalog; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA catalog;


--
-- Name: aplicar_delta_stock_ubicacion(); Type: FUNCTION; Schema: catalog; Owner: -
--

CREATE FUNCTION catalog.aplicar_delta_stock_ubicacion() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE
  v_delta      integer := NEW.stock - OLD.stock;
  v_default    uuid;
  v_ubicacion  uuid;
  v_stock      integer;
  v_quitar     integer;
  v_restante   integer;
BEGIN
  IF v_delta = 0 THEN
    RETURN NEW;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM catalog.item_stock_ubicacion WHERE item_id = NEW.id) THEN
    RETURN NEW;
  END IF;
  IF current_setting('catalog.stock_sync', true) = 'on' THEN
    RETURN NEW;
  END IF;

  PERFORM set_config('catalog.stock_sync', 'on', true);

  SELECT u.id INTO v_default
    FROM catalog.ubicaciones u
   WHERE u.empresa_id = NEW.empresa_id AND u.activo AND u.predeterminada
   LIMIT 1;

  IF v_delta > 0 THEN
    IF v_default IS NULL THEN
      SELECT ubicacion_id INTO v_default
        FROM catalog.item_stock_ubicacion
       WHERE item_id = NEW.id
       ORDER BY stock DESC, ubicacion_id
       LIMIT 1;
    END IF;
    INSERT INTO catalog.item_stock_ubicacion AS s
      (empresa_id, item_id, ubicacion_id, stock)
    VALUES (NEW.empresa_id, NEW.id, v_default, v_delta)
    ON CONFLICT (item_id, ubicacion_id) DO UPDATE
      SET stock = s.stock + EXCLUDED.stock,
          actualizado_en = NOW();
  ELSE
    v_restante := -v_delta;
    FOR v_ubicacion, v_stock IN
      SELECT ubicacion_id, stock
        FROM catalog.item_stock_ubicacion
       WHERE item_id = NEW.id AND stock > 0
       ORDER BY (ubicacion_id = COALESCE(v_default, ubicacion_id)) DESC,
                stock DESC,
                ubicacion_id
       FOR UPDATE
    LOOP
      EXIT WHEN v_restante <= 0;
      v_quitar := LEAST(v_stock, v_restante);
      UPDATE catalog.item_stock_ubicacion
         SET stock = stock - v_quitar,
             actualizado_en = NOW()
       WHERE item_id = NEW.id AND ubicacion_id = v_ubicacion;
      v_restante := v_restante - v_quitar;
    END LOOP;
    IF v_restante > 0 THEN
      RAISE EXCEPTION 'STOCK_UBICACION_INCONSISTENTE'
        USING DETAIL = 'El stock fisico del item no alcanza en sus ubicaciones.';
    END IF;
  END IF;

  PERFORM set_config('catalog.stock_sync', 'off', true);
  RETURN NEW;
END;
$$;


--
-- Name: crear_stock_ubicacion_inicial(); Type: FUNCTION; Schema: catalog; Owner: -
--

CREATE FUNCTION catalog.crear_stock_ubicacion_inicial() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE
  v_ubicacion uuid;
BEGIN
  IF NOT NEW.track_inventory OR NEW.eliminado_en IS NOT NULL THEN
    RETURN NEW;
  END IF;
  SELECT id INTO v_ubicacion
    FROM catalog.ubicaciones
   WHERE empresa_id = NEW.empresa_id AND activo AND predeterminada
   LIMIT 1;
  IF v_ubicacion IS NULL THEN
    RETURN NEW;
  END IF;
  INSERT INTO catalog.item_stock_ubicacion (empresa_id, item_id, ubicacion_id, stock)
  VALUES (NEW.empresa_id, NEW.id, v_ubicacion, COALESCE(NEW.stock, 0))
  ON CONFLICT (item_id, ubicacion_id) DO NOTHING;
  RETURN NEW;
END;
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: ajustes_stock; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.ajustes_stock (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    cantidad_anterior integer,
    cantidad_nueva integer NOT NULL,
    motivo text NOT NULL,
    realizado_en timestamp with time zone DEFAULT now() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid
);


--
-- Name: configuracion_inventario; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.configuracion_inventario (
    empresa_id uuid NOT NULL,
    metodo_costeo text DEFAULT 'promedio'::text NOT NULL,
    permitir_stock_negativo boolean DEFAULT false NOT NULL,
    alerta_stock_bajo boolean DEFAULT true NOT NULL,
    actualizar_costo_en_compra boolean DEFAULT true NOT NULL,
    unidad_defecto text DEFAULT 'Unidad (u)'::text NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT configuracion_inventario_metodo_costeo_check CHECK ((metodo_costeo = ANY (ARRAY['promedio'::text, 'fifo'::text, 'ueps'::text])))
);


--
-- Name: eventos_procesados; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.eventos_procesados (
    event_id uuid NOT NULL,
    tipo text NOT NULL,
    procesado_en timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: historico_precios; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.historico_precios (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    precio_anterior_cents integer,
    precio_nuevo_cents integer NOT NULL,
    vigente_desde timestamp with time zone DEFAULT now() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid
);


--
-- Name: item_atributo_esquemas; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.item_atributo_esquemas (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid,
    tipo text NOT NULL,
    categoria text NOT NULL,
    esquema jsonb NOT NULL,
    version integer DEFAULT 1 NOT NULL,
    activo boolean DEFAULT true NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT item_atributo_esquemas_tipo_check CHECK ((tipo = ANY (ARRAY['producto'::text, 'servicio'::text, 'alquiler'::text]))),
    CONSTRAINT item_atributo_esquemas_version_check CHECK ((version >= 1))
);


--
-- Name: item_componentes; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.item_componentes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid NOT NULL,
    componente_item_id uuid NOT NULL,
    cantidad integer NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT item_componentes_cantidad_check CHECK ((cantidad > 0)),
    CONSTRAINT item_componentes_distintos CHECK ((item_id <> componente_item_id))
);


--
-- Name: item_imagenes; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.item_imagenes (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid NOT NULL,
    objeto_id uuid NOT NULL,
    orden smallint NOT NULL,
    es_principal boolean DEFAULT false NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT item_imagenes_orden_check CHECK (((orden >= 0) AND (orden <= 2)))
);


--
-- Name: item_proveedores; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.item_proveedores (
    item_id uuid NOT NULL,
    proveedor_id uuid NOT NULL,
    codigo_proveedor text DEFAULT ''::text NOT NULL,
    costo_cents integer,
    es_preferido boolean DEFAULT false NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT item_proveedores_costo_cents_check CHECK (((costo_cents IS NULL) OR (costo_cents >= 0)))
);


--
-- Name: item_series; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.item_series (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid NOT NULL,
    ubicacion_id uuid,
    serial text NOT NULL,
    imei text DEFAULT ''::text NOT NULL,
    estado text DEFAULT 'disponible'::text NOT NULL,
    order_id uuid,
    vendido_en timestamp with time zone,
    garantia_hasta date,
    devuelto_en timestamp with time zone,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT item_series_estado_check CHECK ((estado = ANY (ARRAY['disponible'::text, 'reservado'::text, 'vendido'::text, 'devuelto'::text, 'baja'::text]))),
    CONSTRAINT item_series_metadata_check CHECK ((jsonb_typeof(metadata) = 'object'::text)),
    CONSTRAINT item_series_serial_check CHECK (((char_length(TRIM(BOTH FROM serial)) >= 1) AND (char_length(TRIM(BOTH FROM serial)) <= 120)))
);


--
-- Name: TABLE item_series; Type: COMMENT; Schema: catalog; Owner: -
--

COMMENT ON TABLE catalog.item_series IS 'Una fila por unidad serializada; conserva venta, devolucion y garantia.';


--
-- Name: item_stock_ubicacion; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.item_stock_ubicacion (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid NOT NULL,
    ubicacion_id uuid NOT NULL,
    stock integer DEFAULT 0 NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT item_stock_ubicacion_stock_check CHECK ((stock >= 0))
);


--
-- Name: TABLE item_stock_ubicacion; Type: COMMENT; Schema: catalog; Owner: -
--

COMMENT ON TABLE catalog.item_stock_ubicacion IS 'Desglose del stock por item y ubicacion; el total sigue en catalog.items.stock.';


--
-- Name: item_tarifas; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.item_tarifas (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid NOT NULL,
    desde date NOT NULL,
    hasta date NOT NULL,
    precio_cents integer NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT item_tarifas_precio_cents_check CHECK ((precio_cents >= 0)),
    CONSTRAINT item_tarifas_rango_check CHECK ((hasta >= desde))
);


--
-- Name: items; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.items (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    tipo text NOT NULL,
    codigo text NOT NULL,
    nombre text NOT NULL,
    descripcion text DEFAULT ''::text NOT NULL,
    categoria text DEFAULT ''::text NOT NULL,
    precio_cents integer NOT NULL,
    costo_cents integer DEFAULT 0 NOT NULL,
    track_inventory boolean DEFAULT false NOT NULL,
    stock integer,
    duration_minutes integer,
    atributos jsonb DEFAULT '{}'::jsonb NOT NULL,
    estado text DEFAULT 'activo'::text NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    eliminado_en timestamp with time zone,
    parent_item_id uuid,
    marca text DEFAULT ''::text NOT NULL,
    codigo_barras text DEFAULT ''::text NOT NULL,
    unidad text DEFAULT 'Unidad (u)'::text NOT NULL,
    permite_fracciones boolean DEFAULT false NOT NULL,
    unidad_compra text DEFAULT 'Igual que la de venta'::text NOT NULL,
    contenido_cantidad integer DEFAULT 0 NOT NULL,
    contenido_unidad text DEFAULT 'Sin contenido'::text NOT NULL,
    tipo_precio text DEFAULT 'fijo'::text NOT NULL,
    tipo_costo text DEFAULT 'fijo'::text NOT NULL,
    stock_minimo integer DEFAULT 0 NOT NULL,
    stock_maximo integer,
    garantia_meses integer,
    es_materia_prima boolean DEFAULT false NOT NULL,
    ubicacion text DEFAULT ''::text NOT NULL,
    track_serial boolean DEFAULT false NOT NULL,
    es_combo boolean DEFAULT false NOT NULL,
    CONSTRAINT items_check CHECK ((((tipo = 'servicio'::text) AND (duration_minutes IS NOT NULL)) OR ((tipo <> 'servicio'::text) AND (duration_minutes IS NULL)))),
    CONSTRAINT items_contenido_cantidad_check CHECK ((contenido_cantidad >= 0)),
    CONSTRAINT items_costo_cents_check CHECK ((costo_cents >= 0)),
    CONSTRAINT items_duration_minutes_check CHECK (((duration_minutes IS NULL) OR ((duration_minutes >= 1) AND (duration_minutes <= 1440)))),
    CONSTRAINT items_estado_check CHECK ((estado = ANY (ARRAY['activo'::text, 'inactivo'::text]))),
    CONSTRAINT items_garantia_meses_check CHECK (((garantia_meses IS NULL) OR (garantia_meses >= 0))),
    CONSTRAINT items_precio_cents_check CHECK ((precio_cents >= 0)),
    CONSTRAINT items_stock_check CHECK (((stock IS NULL) OR (stock >= 0))),
    CONSTRAINT items_stock_maximo_check CHECK (((stock_maximo IS NULL) OR (stock_maximo >= 0))),
    CONSTRAINT items_stock_minimo_check CHECK ((stock_minimo >= 0)),
    CONSTRAINT items_tipo_check CHECK ((tipo = ANY (ARRAY['producto'::text, 'servicio'::text, 'alquiler'::text]))),
    CONSTRAINT items_tipo_costo_check CHECK ((tipo_costo = ANY (ARRAY['fijo'::text, 'escala'::text]))),
    CONSTRAINT items_tipo_precio_check CHECK ((tipo_precio = ANY (ARRAY['fijo'::text, 'escala'::text]))),
    CONSTRAINT items_track_serial_check CHECK (((NOT track_serial) OR ((tipo = 'producto'::text) AND track_inventory)))
);


--
-- Name: items_vista; Type: VIEW; Schema: catalog; Owner: -
--

CREATE VIEW catalog.items_vista AS
 SELECT id AS item_id,
    empresa_id,
    tipo,
    codigo,
    nombre,
    descripcion,
    categoria,
    precio_cents,
    costo_cents,
    track_inventory,
    stock,
    duration_minutes,
    atributos,
    estado,
    parent_item_id,
    marca,
    codigo_barras,
    unidad,
    permite_fracciones,
    unidad_compra,
    contenido_cantidad,
    contenido_unidad,
    tipo_precio,
    tipo_costo,
    stock_minimo,
    stock_maximo,
    garantia_meses,
    es_materia_prima,
    ubicacion,
    track_serial,
    es_combo,
    creado_en,
    actualizado_en
   FROM catalog.items i
  WHERE (eliminado_en IS NULL);


--
-- Name: lista_precio_items; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.lista_precio_items (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    lista_id uuid,
    item_id uuid NOT NULL,
    desde_cantidad integer DEFAULT 1 NOT NULL,
    hasta_cantidad integer,
    precio_cents integer NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lista_precio_items_check CHECK (((hasta_cantidad IS NULL) OR (hasta_cantidad >= desde_cantidad))),
    CONSTRAINT lista_precio_items_desde_cantidad_check CHECK ((desde_cantidad >= 1)),
    CONSTRAINT lista_precio_items_precio_cents_check CHECK ((precio_cents >= 0))
);


--
-- Name: TABLE lista_precio_items; Type: COMMENT; Schema: catalog; Owner: -
--

COMMENT ON TABLE catalog.lista_precio_items IS 'Precio por item y rango de cantidad; lista_id NULL es la escala base que activa tipo_precio = escala.';


--
-- Name: listas_precio; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.listas_precio (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    codigo text NOT NULL,
    nombre text NOT NULL,
    descripcion text DEFAULT ''::text NOT NULL,
    moneda text DEFAULT 'NIO'::text NOT NULL,
    segmento_codigo text DEFAULT ''::text NOT NULL,
    predeterminada boolean DEFAULT false NOT NULL,
    activo boolean DEFAULT true NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT listas_precio_codigo_check CHECK ((codigo ~ '^[a-z][a-z0-9_-]{1,39}$'::text)),
    CONSTRAINT listas_precio_moneda_check CHECK ((moneda ~ '^[A-Z]{3}$'::text)),
    CONSTRAINT listas_precio_nombre_check CHECK (((char_length(TRIM(BOTH FROM nombre)) >= 2) AND (char_length(TRIM(BOTH FROM nombre)) <= 120))),
    CONSTRAINT listas_precio_segmento_codigo_check CHECK (((segmento_codigo = ''::text) OR (segmento_codigo ~ '^[a-z][a-z0-9_-]{0,39}$'::text)))
);


--
-- Name: TABLE listas_precio; Type: COMMENT; Schema: catalog; Owner: -
--

COMMENT ON TABLE catalog.listas_precio IS 'Listas de precio por empresa y segmento; predeterminada = lista aplicada sin seleccion explicita.';


--
-- Name: lotes_inventario; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.lotes_inventario (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid NOT NULL,
    proveedor_id uuid,
    codigo_lote text NOT NULL,
    vence_en date,
    cantidad_inicial integer NOT NULL,
    cantidad_disponible integer NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT lotes_inventario_cantidad_disponible_check CHECK ((cantidad_disponible >= 0)),
    CONSTRAINT lotes_inventario_cantidad_inicial_check CHECK ((cantidad_inicial >= 0)),
    CONSTRAINT lotes_inventario_check CHECK ((cantidad_disponible <= cantidad_inicial))
);


--
-- Name: movimientos_inventario; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.movimientos_inventario (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    tipo text NOT NULL,
    motivo text DEFAULT ''::text NOT NULL,
    referencia text DEFAULT ''::text NOT NULL,
    ubicacion_origen text DEFAULT ''::text NOT NULL,
    ubicacion_destino text DEFAULT ''::text NOT NULL,
    lineas jsonb DEFAULT '[]'::jsonb NOT NULL,
    creado_por text,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT movimientos_inventario_tipo_check CHECK ((tipo = ANY (ARRAY['entrada'::text, 'salida'::text, 'ajuste'::text, 'traslado'::text])))
);


--
-- Name: outbox; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.outbox (
    event_id uuid NOT NULL,
    tipo text NOT NULL,
    payload jsonb NOT NULL,
    estado text DEFAULT 'pendiente'::text NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    publicado_en timestamp with time zone,
    bloqueado_en timestamp with time zone,
    CONSTRAINT outbox_estado_check CHECK ((estado = ANY (ARRAY['pendiente'::text, 'publicado'::text])))
);


--
-- Name: promociones; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.promociones (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    nombre text NOT NULL,
    tipo text NOT NULL,
    sku text DEFAULT ''::text NOT NULL,
    cantidad_requerida integer,
    cantidad_gratis integer,
    descuento_porcentaje integer,
    fecha_desde date NOT NULL,
    fecha_hasta date NOT NULL,
    estado text DEFAULT 'activo'::text NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    eliminado_en timestamp with time zone,
    item_id uuid,
    CONSTRAINT promociones_cantidad_gratis_check CHECK (((cantidad_gratis IS NULL) OR (cantidad_gratis >= 1))),
    CONSTRAINT promociones_cantidad_requerida_check CHECK (((cantidad_requerida IS NULL) OR (cantidad_requerida >= 1))),
    CONSTRAINT promociones_check CHECK ((fecha_hasta >= fecha_desde)),
    CONSTRAINT promociones_descuento_porcentaje_check CHECK (((descuento_porcentaje IS NULL) OR ((descuento_porcentaje >= 0) AND (descuento_porcentaje <= 100)))),
    CONSTRAINT promociones_estado_check CHECK ((estado = ANY (ARRAY['activo'::text, 'inactivo'::text]))),
    CONSTRAINT promociones_tipo_check CHECK ((tipo = ANY (ARRAY['2x1'::text, 'descuento_cantidad'::text, 'descuento_porcentaje'::text, 'regalo'::text])))
);


--
-- Name: proveedores; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.proveedores (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    codigo text NOT NULL,
    nombre text NOT NULL,
    correo text DEFAULT ''::text NOT NULL,
    telefono text DEFAULT ''::text NOT NULL,
    activo boolean DEFAULT true NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: reservas_lotes; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.reservas_lotes (
    order_id uuid NOT NULL,
    lote_id uuid NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid NOT NULL,
    cantidad integer NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT reservas_lotes_cantidad_check CHECK ((cantidad > 0))
);


--
-- Name: reservas_ordenes; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.reservas_ordenes (
    order_id uuid NOT NULL,
    items_json jsonb NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: ubicaciones; Type: TABLE; Schema: catalog; Owner: -
--

CREATE TABLE catalog.ubicaciones (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    codigo text NOT NULL,
    nombre text NOT NULL,
    tipo text DEFAULT 'bodega'::text NOT NULL,
    direccion text DEFAULT ''::text NOT NULL,
    predeterminada boolean DEFAULT false NOT NULL,
    activo boolean DEFAULT true NOT NULL,
    creado_en timestamp with time zone DEFAULT now() NOT NULL,
    actualizado_en timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ubicaciones_codigo_check CHECK ((codigo ~ '^[a-z][a-z0-9_-]{1,39}$'::text)),
    CONSTRAINT ubicaciones_nombre_check CHECK (((char_length(TRIM(BOTH FROM nombre)) >= 2) AND (char_length(TRIM(BOTH FROM nombre)) <= 120))),
    CONSTRAINT ubicaciones_tipo_check CHECK ((tipo = ANY (ARRAY['bodega'::text, 'sucursal'::text, 'almacen'::text, 'otro'::text])))
);


--
-- Name: TABLE ubicaciones; Type: COMMENT; Schema: catalog; Owner: -
--

COMMENT ON TABLE catalog.ubicaciones IS 'Bodegas, sucursales o almacenes del tenant; una puede ser la predeterminada.';


--
-- Name: ajustes_stock ajustes_stock_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.ajustes_stock
    ADD CONSTRAINT ajustes_stock_pkey PRIMARY KEY (id);


--
-- Name: configuracion_inventario configuracion_inventario_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.configuracion_inventario
    ADD CONSTRAINT configuracion_inventario_pkey PRIMARY KEY (empresa_id);


--
-- Name: eventos_procesados eventos_procesados_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.eventos_procesados
    ADD CONSTRAINT eventos_procesados_pkey PRIMARY KEY (event_id);


--
-- Name: historico_precios historico_precios_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.historico_precios
    ADD CONSTRAINT historico_precios_pkey PRIMARY KEY (id);


--
-- Name: item_atributo_esquemas item_atributo_esquemas_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_atributo_esquemas
    ADD CONSTRAINT item_atributo_esquemas_pkey PRIMARY KEY (id);


--
-- Name: item_componentes item_componentes_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_componentes
    ADD CONSTRAINT item_componentes_pkey PRIMARY KEY (id);


--
-- Name: item_componentes item_componentes_unique; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_componentes
    ADD CONSTRAINT item_componentes_unique UNIQUE (item_id, componente_item_id);


--
-- Name: item_imagenes item_imagenes_item_id_objeto_id_key; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_imagenes
    ADD CONSTRAINT item_imagenes_item_id_objeto_id_key UNIQUE (item_id, objeto_id);


--
-- Name: item_imagenes item_imagenes_item_id_orden_key; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_imagenes
    ADD CONSTRAINT item_imagenes_item_id_orden_key UNIQUE (item_id, orden);


--
-- Name: item_imagenes item_imagenes_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_imagenes
    ADD CONSTRAINT item_imagenes_pkey PRIMARY KEY (id);


--
-- Name: item_proveedores item_proveedores_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_proveedores
    ADD CONSTRAINT item_proveedores_pkey PRIMARY KEY (item_id, proveedor_id);


--
-- Name: item_series item_series_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_series
    ADD CONSTRAINT item_series_pkey PRIMARY KEY (id);


--
-- Name: item_stock_ubicacion item_stock_ubicacion_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_stock_ubicacion
    ADD CONSTRAINT item_stock_ubicacion_pkey PRIMARY KEY (id);


--
-- Name: item_stock_ubicacion item_stock_ubicacion_unique; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_stock_ubicacion
    ADD CONSTRAINT item_stock_ubicacion_unique UNIQUE (item_id, ubicacion_id);


--
-- Name: item_tarifas item_tarifas_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_tarifas
    ADD CONSTRAINT item_tarifas_pkey PRIMARY KEY (id);


--
-- Name: item_tarifas item_tarifas_sin_solape; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_tarifas
    ADD CONSTRAINT item_tarifas_sin_solape EXCLUDE USING gist (item_id WITH =, daterange(desde, hasta, '[]'::text) WITH &&);


--
-- Name: items items_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.items
    ADD CONSTRAINT items_pkey PRIMARY KEY (id);


--
-- Name: lista_precio_items lista_precio_items_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lista_precio_items
    ADD CONSTRAINT lista_precio_items_pkey PRIMARY KEY (id);


--
-- Name: lista_precio_items lista_precio_items_sin_solape_base; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lista_precio_items
    ADD CONSTRAINT lista_precio_items_sin_solape_base EXCLUDE USING gist (empresa_id WITH =, item_id WITH =, int4range(desde_cantidad, hasta_cantidad, '[]'::text) WITH &&) WHERE ((lista_id IS NULL));


--
-- Name: lista_precio_items lista_precio_items_sin_solape_lista; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lista_precio_items
    ADD CONSTRAINT lista_precio_items_sin_solape_lista EXCLUDE USING gist (lista_id WITH =, item_id WITH =, int4range(desde_cantidad, hasta_cantidad, '[]'::text) WITH &&) WHERE ((lista_id IS NOT NULL));


--
-- Name: listas_precio listas_precio_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.listas_precio
    ADD CONSTRAINT listas_precio_pkey PRIMARY KEY (id);


--
-- Name: lotes_inventario lotes_inventario_empresa_id_item_id_codigo_lote_key; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lotes_inventario
    ADD CONSTRAINT lotes_inventario_empresa_id_item_id_codigo_lote_key UNIQUE (empresa_id, item_id, codigo_lote);


--
-- Name: lotes_inventario lotes_inventario_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lotes_inventario
    ADD CONSTRAINT lotes_inventario_pkey PRIMARY KEY (id);


--
-- Name: movimientos_inventario movimientos_inventario_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.movimientos_inventario
    ADD CONSTRAINT movimientos_inventario_pkey PRIMARY KEY (id);


--
-- Name: outbox outbox_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.outbox
    ADD CONSTRAINT outbox_pkey PRIMARY KEY (event_id);


--
-- Name: promociones promociones_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.promociones
    ADD CONSTRAINT promociones_pkey PRIMARY KEY (id);


--
-- Name: proveedores proveedores_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.proveedores
    ADD CONSTRAINT proveedores_pkey PRIMARY KEY (id);


--
-- Name: reservas_lotes reservas_lotes_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.reservas_lotes
    ADD CONSTRAINT reservas_lotes_pkey PRIMARY KEY (order_id, lote_id);


--
-- Name: reservas_ordenes reservas_ordenes_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.reservas_ordenes
    ADD CONSTRAINT reservas_ordenes_pkey PRIMARY KEY (order_id);


--
-- Name: ubicaciones ubicaciones_pkey; Type: CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.ubicaciones
    ADD CONSTRAINT ubicaciones_pkey PRIMARY KEY (id);


--
-- Name: idx_catalog_outbox_claim; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_catalog_outbox_claim ON catalog.outbox USING btree (estado, creado_en) WHERE (estado = 'pendiente'::text);


--
-- Name: idx_catalog_outbox_pendientes; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_catalog_outbox_pendientes ON catalog.outbox USING btree (estado, creado_en) WHERE (estado = 'pendiente'::text);


--
-- Name: idx_item_atributo_esquemas_lookup; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_atributo_esquemas_lookup ON catalog.item_atributo_esquemas USING btree (empresa_id, tipo, lower(categoria), activo);


--
-- Name: idx_item_componentes_empresa; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_componentes_empresa ON catalog.item_componentes USING btree (empresa_id, item_id);


--
-- Name: idx_item_imagenes_empresa; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_imagenes_empresa ON catalog.item_imagenes USING btree (empresa_id, item_id);


--
-- Name: idx_item_series_item_estado; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_series_item_estado ON catalog.item_series USING btree (empresa_id, item_id, estado);


--
-- Name: idx_item_series_order; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_series_order ON catalog.item_series USING btree (order_id) WHERE (order_id IS NOT NULL);


--
-- Name: idx_item_stock_ubicacion_item; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_stock_ubicacion_item ON catalog.item_stock_ubicacion USING btree (empresa_id, item_id);


--
-- Name: idx_item_stock_ubicacion_ubicacion; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_stock_ubicacion_ubicacion ON catalog.item_stock_ubicacion USING btree (ubicacion_id, item_id);


--
-- Name: idx_item_tarifas_item; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_item_tarifas_item ON catalog.item_tarifas USING btree (empresa_id, item_id, desde);


--
-- Name: idx_items_empresa_tipo_estado; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_items_empresa_tipo_estado ON catalog.items USING btree (empresa_id, tipo, estado, creado_en DESC) WHERE (eliminado_en IS NULL);


--
-- Name: idx_items_es_combo; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_items_es_combo ON catalog.items USING btree (empresa_id, creado_en DESC) WHERE (es_combo AND (eliminado_en IS NULL));


--
-- Name: idx_items_parent; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_items_parent ON catalog.items USING btree (parent_item_id, estado) WHERE (eliminado_en IS NULL);


--
-- Name: idx_lista_precio_items_base; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_lista_precio_items_base ON catalog.lista_precio_items USING btree (empresa_id, item_id, desde_cantidad) WHERE (lista_id IS NULL);


--
-- Name: idx_lista_precio_items_lista; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_lista_precio_items_lista ON catalog.lista_precio_items USING btree (lista_id, item_id, desde_cantidad);


--
-- Name: idx_listas_precio_empresa; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_listas_precio_empresa ON catalog.listas_precio USING btree (empresa_id, activo, nombre);


--
-- Name: idx_lotes_empresa_vencimiento; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_lotes_empresa_vencimiento ON catalog.lotes_inventario USING btree (empresa_id, vence_en, codigo_lote);


--
-- Name: idx_movimientos_empresa_fecha; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_movimientos_empresa_fecha ON catalog.movimientos_inventario USING btree (empresa_id, creado_en DESC);


--
-- Name: idx_movimientos_empresa_tipo; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_movimientos_empresa_tipo ON catalog.movimientos_inventario USING btree (empresa_id, tipo, creado_en DESC);


--
-- Name: idx_promociones_empresa_estado; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_promociones_empresa_estado ON catalog.promociones USING btree (empresa_id, estado, fecha_desde, fecha_hasta) WHERE (eliminado_en IS NULL);


--
-- Name: idx_reservas_lotes_empresa_item; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_reservas_lotes_empresa_item ON catalog.reservas_lotes USING btree (empresa_id, item_id);


--
-- Name: idx_ubicaciones_empresa; Type: INDEX; Schema: catalog; Owner: -
--

CREATE INDEX idx_ubicaciones_empresa ON catalog.ubicaciones USING btree (empresa_id, activo, nombre);


--
-- Name: ux_item_atributo_esquemas_scope; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_item_atributo_esquemas_scope ON catalog.item_atributo_esquemas USING btree (COALESCE(empresa_id, '00000000-0000-0000-0000-000000000000'::uuid), tipo, lower(categoria), version);


--
-- Name: ux_item_imagen_principal; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_item_imagen_principal ON catalog.item_imagenes USING btree (item_id) WHERE es_principal;


--
-- Name: ux_item_series_empresa_imei; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_item_series_empresa_imei ON catalog.item_series USING btree (empresa_id, imei) WHERE (imei <> ''::text);


--
-- Name: ux_item_series_empresa_serial; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_item_series_empresa_serial ON catalog.item_series USING btree (empresa_id, lower(serial));


--
-- Name: ux_items_empresa_barcode; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_items_empresa_barcode ON catalog.items USING btree (empresa_id, codigo_barras) WHERE ((codigo_barras <> ''::text) AND (eliminado_en IS NULL));


--
-- Name: ux_items_empresa_tipo_codigo; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_items_empresa_tipo_codigo ON catalog.items USING btree (empresa_id, tipo, upper(codigo)) WHERE (eliminado_en IS NULL);


--
-- Name: ux_lista_precio_items_base; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_lista_precio_items_base ON catalog.lista_precio_items USING btree (empresa_id, item_id, desde_cantidad) WHERE (lista_id IS NULL);


--
-- Name: ux_lista_precio_items_lista; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_lista_precio_items_lista ON catalog.lista_precio_items USING btree (lista_id, item_id, desde_cantidad) WHERE (lista_id IS NOT NULL);


--
-- Name: ux_listas_precio_empresa_codigo; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_listas_precio_empresa_codigo ON catalog.listas_precio USING btree (empresa_id, lower(codigo));


--
-- Name: ux_listas_precio_predeterminada; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_listas_precio_predeterminada ON catalog.listas_precio USING btree (empresa_id) WHERE (predeterminada AND activo);


--
-- Name: ux_listas_precio_segmento; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_listas_precio_segmento ON catalog.listas_precio USING btree (empresa_id, segmento_codigo) WHERE ((segmento_codigo <> ''::text) AND activo);


--
-- Name: ux_proveedores_empresa_codigo; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_proveedores_empresa_codigo ON catalog.proveedores USING btree (empresa_id, upper(codigo));


--
-- Name: ux_ubicaciones_empresa_codigo; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_ubicaciones_empresa_codigo ON catalog.ubicaciones USING btree (empresa_id, lower(codigo));


--
-- Name: ux_ubicaciones_predeterminada; Type: INDEX; Schema: catalog; Owner: -
--

CREATE UNIQUE INDEX ux_ubicaciones_predeterminada ON catalog.ubicaciones USING btree (empresa_id) WHERE (predeterminada AND activo);


--
-- Name: items trg_items_delta_stock; Type: TRIGGER; Schema: catalog; Owner: -
--

CREATE TRIGGER trg_items_delta_stock AFTER UPDATE OF stock ON catalog.items FOR EACH ROW EXECUTE FUNCTION catalog.aplicar_delta_stock_ubicacion();


--
-- Name: items trg_items_stock_ubicacion_inicial; Type: TRIGGER; Schema: catalog; Owner: -
--

CREATE TRIGGER trg_items_stock_ubicacion_inicial AFTER INSERT ON catalog.items FOR EACH ROW EXECUTE FUNCTION catalog.crear_stock_ubicacion_inicial();


--
-- Name: ajustes_stock ajustes_stock_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.ajustes_stock
    ADD CONSTRAINT ajustes_stock_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: historico_precios historico_precios_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.historico_precios
    ADD CONSTRAINT historico_precios_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: item_componentes item_componentes_componente_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_componentes
    ADD CONSTRAINT item_componentes_componente_item_id_fkey FOREIGN KEY (componente_item_id) REFERENCES catalog.items(id) ON DELETE RESTRICT;


--
-- Name: item_componentes item_componentes_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_componentes
    ADD CONSTRAINT item_componentes_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: item_imagenes item_imagenes_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_imagenes
    ADD CONSTRAINT item_imagenes_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: item_proveedores item_proveedores_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_proveedores
    ADD CONSTRAINT item_proveedores_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: item_proveedores item_proveedores_proveedor_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_proveedores
    ADD CONSTRAINT item_proveedores_proveedor_id_fkey FOREIGN KEY (proveedor_id) REFERENCES catalog.proveedores(id) ON DELETE CASCADE;


--
-- Name: item_series item_series_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_series
    ADD CONSTRAINT item_series_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: item_series item_series_ubicacion_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_series
    ADD CONSTRAINT item_series_ubicacion_id_fkey FOREIGN KEY (ubicacion_id) REFERENCES catalog.ubicaciones(id) ON DELETE SET NULL;


--
-- Name: item_stock_ubicacion item_stock_ubicacion_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_stock_ubicacion
    ADD CONSTRAINT item_stock_ubicacion_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: item_stock_ubicacion item_stock_ubicacion_ubicacion_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_stock_ubicacion
    ADD CONSTRAINT item_stock_ubicacion_ubicacion_id_fkey FOREIGN KEY (ubicacion_id) REFERENCES catalog.ubicaciones(id) ON DELETE CASCADE;


--
-- Name: item_tarifas item_tarifas_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.item_tarifas
    ADD CONSTRAINT item_tarifas_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: items items_parent_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.items
    ADD CONSTRAINT items_parent_item_id_fkey FOREIGN KEY (parent_item_id) REFERENCES catalog.items(id);


--
-- Name: lista_precio_items lista_precio_items_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lista_precio_items
    ADD CONSTRAINT lista_precio_items_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE CASCADE;


--
-- Name: lista_precio_items lista_precio_items_lista_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lista_precio_items
    ADD CONSTRAINT lista_precio_items_lista_id_fkey FOREIGN KEY (lista_id) REFERENCES catalog.listas_precio(id) ON DELETE CASCADE;


--
-- Name: lotes_inventario lotes_inventario_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lotes_inventario
    ADD CONSTRAINT lotes_inventario_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id);


--
-- Name: lotes_inventario lotes_inventario_proveedor_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.lotes_inventario
    ADD CONSTRAINT lotes_inventario_proveedor_id_fkey FOREIGN KEY (proveedor_id) REFERENCES catalog.proveedores(id);


--
-- Name: promociones promociones_item_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.promociones
    ADD CONSTRAINT promociones_item_id_fkey FOREIGN KEY (item_id) REFERENCES catalog.items(id) ON DELETE SET NULL;


--
-- Name: reservas_lotes reservas_lotes_lote_id_fkey; Type: FK CONSTRAINT; Schema: catalog; Owner: -
--

ALTER TABLE ONLY catalog.reservas_lotes
    ADD CONSTRAINT reservas_lotes_lote_id_fkey FOREIGN KEY (lote_id) REFERENCES catalog.lotes_inventario(id) ON DELETE CASCADE;


--
-- Name: item_atributo_esquemas; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.item_atributo_esquemas ENABLE ROW LEVEL SECURITY;

--
-- Name: item_componentes; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.item_componentes ENABLE ROW LEVEL SECURITY;

--
-- Name: item_componentes item_componentes_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_componentes item_componentes_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_componentes item_componentes_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_proveedores; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.item_proveedores ENABLE ROW LEVEL SECURITY;

--
-- Name: item_proveedores item_proveedores_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_proveedores item_proveedores_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_atributo_esquemas item_schemas_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_atributo_esquemas item_schemas_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_atributo_esquemas item_schemas_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_series; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.item_series ENABLE ROW LEVEL SECURITY;

--
-- Name: item_series item_series_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_series item_series_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_series item_series_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_stock_ubicacion; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.item_stock_ubicacion ENABLE ROW LEVEL SECURITY;

--
-- Name: item_stock_ubicacion item_stock_ubicacion_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_stock_ubicacion item_stock_ubicacion_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_stock_ubicacion item_stock_ubicacion_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_tarifas; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.item_tarifas ENABLE ROW LEVEL SECURITY;

--
-- Name: item_tarifas item_tarifas_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_tarifas item_tarifas_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: item_tarifas item_tarifas_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: items; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.items ENABLE ROW LEVEL SECURITY;

--
-- Name: items items_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: items items_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: items items_empresa_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: lista_precio_items; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.lista_precio_items ENABLE ROW LEVEL SECURITY;

--
-- Name: lista_precio_items lista_precio_items_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: lista_precio_items lista_precio_items_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: lista_precio_items lista_precio_items_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: listas_precio; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.listas_precio ENABLE ROW LEVEL SECURITY;

--
-- Name: listas_precio listas_precio_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: listas_precio listas_precio_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: listas_precio listas_precio_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: lotes_inventario lotes_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: lotes_inventario lotes_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: lotes_inventario; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.lotes_inventario ENABLE ROW LEVEL SECURITY;

--
-- Name: proveedores; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.proveedores ENABLE ROW LEVEL SECURITY;

--
-- Name: proveedores proveedores_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: proveedores proveedores_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: ubicaciones; Type: ROW SECURITY; Schema: catalog; Owner: -
--

ALTER TABLE catalog.ubicaciones ENABLE ROW LEVEL SECURITY;

--
-- Name: ubicaciones ubicaciones_admin; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: ubicaciones ubicaciones_empresa; Type: POLICY; Schema: catalog; Owner: -
--



-- Name: ubicaciones ubicaciones_lectura; Type: POLICY; Schema: catalog; Owner: -
--



-- PostgreSQL database dump complete
--

