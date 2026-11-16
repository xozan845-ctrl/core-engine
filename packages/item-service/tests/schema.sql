-- Esquema mínimo de item-service (ADR-0018).
--
-- Solo las tablas que el servicio posee. `inventory.ubicaciones` se incluye
-- porque `item.item_series.ubicacion_id` la referencia; item-service no la
-- escribe y la lee únicamente para validar la ubicación de una serie.

DROP SCHEMA IF EXISTS item CASCADE;
DROP SCHEMA IF EXISTS inventory CASCADE;

CREATE SCHEMA inventory;
--
-- PostgreSQL database dump
--

-- Dumped from database version 16.4
-- Dumped by pg_dump version 16.4

SELECT pg_catalog.set_config('search_path', '', false);

--
-- Name: ubicaciones; Type: TABLE; Schema: inventory; Owner: -
--

CREATE TABLE inventory.ubicaciones (
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
-- Name: ubicaciones ubicaciones_pkey; Type: CONSTRAINT; Schema: inventory; Owner: -
--

ALTER TABLE ONLY inventory.ubicaciones
    ADD CONSTRAINT ubicaciones_pkey PRIMARY KEY (id);

--
-- Name: ubicaciones_empresa_id_activo_nombre_idx; Type: INDEX; Schema: inventory; Owner: -
--

CREATE INDEX ubicaciones_empresa_id_activo_nombre_idx ON inventory.ubicaciones USING btree (empresa_id, activo, nombre);

--
-- Name: ubicaciones_empresa_id_idx; Type: INDEX; Schema: inventory; Owner: -
--

CREATE UNIQUE INDEX ubicaciones_empresa_id_idx ON inventory.ubicaciones USING btree (empresa_id) WHERE (predeterminada AND activo);

--
-- Name: ubicaciones_empresa_id_lower_idx; Type: INDEX; Schema: inventory; Owner: -
--

CREATE UNIQUE INDEX ubicaciones_empresa_id_lower_idx ON inventory.ubicaciones USING btree (empresa_id, lower(codigo));

--
-- Name: ubicaciones; Type: ROW SECURITY; Schema: inventory; Owner: -
--

--
-- Name: ubicaciones ubicaciones_admin; Type: POLICY; Schema: inventory; Owner: -
--

--
-- Name: ubicaciones ubicaciones_empresa; Type: POLICY; Schema: inventory; Owner: -
--

--
-- PostgreSQL database dump complete
--

--
-- PostgreSQL database dump
--

-- Dumped from database version 16.4
-- Dumped by pg_dump version 16.4

SELECT pg_catalog.set_config('search_path', '', false);

--
-- Name: item; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA item;
CREATE SCHEMA IF NOT EXISTS identity;
DROP TABLE IF EXISTS identity.negocios CASCADE;
CREATE TABLE identity.negocios (
    id uuid DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    empresa_id uuid NOT NULL,
    es_predeterminado boolean DEFAULT false NOT NULL
);

--
-- Name: historico_precios; Type: TABLE; Schema: item; Owner: -
--

CREATE TABLE item.historico_precios (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    precio_anterior_cents integer,
    precio_nuevo_cents integer NOT NULL,
    vigente_desde timestamp with time zone DEFAULT now() NOT NULL,
    empresa_id uuid NOT NULL,
    item_id uuid
);

--
-- Name: item_atributo_esquemas; Type: TABLE; Schema: item; Owner: -
--

CREATE TABLE item.item_atributo_esquemas (
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
-- Name: item_componentes; Type: TABLE; Schema: item; Owner: -
--

CREATE TABLE item.item_componentes (
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
-- Name: item_imagenes; Type: TABLE; Schema: item; Owner: -
--

CREATE TABLE item.item_imagenes (
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
-- Name: item_series; Type: TABLE; Schema: item; Owner: -
--

CREATE TABLE item.item_series (
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
-- Name: items; Type: TABLE; Schema: item; Owner: -
--

CREATE TABLE item.items (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    empresa_id uuid NOT NULL,
    negocio_id uuid,
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
-- Name: items_vista; Type: VIEW; Schema: item; Owner: -
--

CREATE VIEW item.items_vista AS
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
    creado_en,
    actualizado_en
   FROM item.items i
  WHERE (eliminado_en IS NULL);

--
-- Name: outbox; Type: TABLE; Schema: item; Owner: -
--

CREATE TABLE item.outbox (
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
-- Name: historico_precios historico_precios_pkey; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.historico_precios
    ADD CONSTRAINT historico_precios_pkey PRIMARY KEY (id);

--
-- Name: item_atributo_esquemas item_atributo_esquemas_pkey; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_atributo_esquemas
    ADD CONSTRAINT item_atributo_esquemas_pkey PRIMARY KEY (id);

--
-- Name: item_componentes item_componentes_item_id_componente_item_id_key; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_componentes
    ADD CONSTRAINT item_componentes_item_id_componente_item_id_key UNIQUE (item_id, componente_item_id);

--
-- Name: item_componentes item_componentes_pkey; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_componentes
    ADD CONSTRAINT item_componentes_pkey PRIMARY KEY (id);

--
-- Name: item_imagenes item_imagenes_item_id_objeto_id_key; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_imagenes
    ADD CONSTRAINT item_imagenes_item_id_objeto_id_key UNIQUE (item_id, objeto_id);

--
-- Name: item_imagenes item_imagenes_item_id_orden_key; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_imagenes
    ADD CONSTRAINT item_imagenes_item_id_orden_key UNIQUE (item_id, orden);

--
-- Name: item_imagenes item_imagenes_pkey; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_imagenes
    ADD CONSTRAINT item_imagenes_pkey PRIMARY KEY (id);

--
-- Name: item_series item_series_pkey; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_series
    ADD CONSTRAINT item_series_pkey PRIMARY KEY (id);

--
-- Name: items items_pkey; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.items
    ADD CONSTRAINT items_pkey PRIMARY KEY (id);

--
-- Name: outbox outbox_pkey; Type: CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.outbox
    ADD CONSTRAINT outbox_pkey PRIMARY KEY (event_id);

--
-- Name: item_atributo_esquemas_coalesce_tipo_lower_version_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE UNIQUE INDEX item_atributo_esquemas_coalesce_tipo_lower_version_idx ON item.item_atributo_esquemas USING btree (COALESCE(empresa_id, '00000000-0000-0000-0000-000000000000'::uuid), tipo, lower(categoria), version);

--
-- Name: item_atributo_esquemas_empresa_id_tipo_lower_activo_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX item_atributo_esquemas_empresa_id_tipo_lower_activo_idx ON item.item_atributo_esquemas USING btree (empresa_id, tipo, lower(categoria), activo);

--
-- Name: item_componentes_empresa_id_item_id_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX item_componentes_empresa_id_item_id_idx ON item.item_componentes USING btree (empresa_id, item_id);

--
-- Name: item_imagenes_empresa_id_item_id_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX item_imagenes_empresa_id_item_id_idx ON item.item_imagenes USING btree (empresa_id, item_id);

--
-- Name: item_imagenes_item_id_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE UNIQUE INDEX item_imagenes_item_id_idx ON item.item_imagenes USING btree (item_id) WHERE es_principal;

--
-- Name: item_series_empresa_id_imei_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE UNIQUE INDEX item_series_empresa_id_imei_idx ON item.item_series USING btree (empresa_id, imei) WHERE (imei <> ''::text);

--
-- Name: item_series_empresa_id_item_id_estado_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX item_series_empresa_id_item_id_estado_idx ON item.item_series USING btree (empresa_id, item_id, estado);

--
-- Name: item_series_empresa_id_lower_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE UNIQUE INDEX item_series_empresa_id_lower_idx ON item.item_series USING btree (empresa_id, lower(serial));

--
-- Name: item_series_order_id_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX item_series_order_id_idx ON item.item_series USING btree (order_id) WHERE (order_id IS NOT NULL);

--
-- Name: items_empresa_id_codigo_barras_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE UNIQUE INDEX items_empresa_id_codigo_barras_idx ON item.items USING btree (empresa_id, codigo_barras) WHERE ((codigo_barras <> ''::text) AND (eliminado_en IS NULL));

--
-- Name: items_empresa_id_creado_en_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX items_empresa_id_creado_en_idx ON item.items USING btree (empresa_id, creado_en DESC) WHERE (es_combo AND (eliminado_en IS NULL));

--
-- Name: items_empresa_id_tipo_estado_creado_en_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX items_empresa_id_tipo_estado_creado_en_idx ON item.items USING btree (empresa_id, tipo, estado, creado_en DESC) WHERE (eliminado_en IS NULL);

--
-- Name: items_empresa_id_tipo_upper_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE UNIQUE INDEX items_empresa_id_tipo_upper_idx ON item.items USING btree (empresa_id, tipo, upper(codigo)) WHERE (eliminado_en IS NULL);

--
-- Name: items_parent_item_id_estado_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX items_parent_item_id_estado_idx ON item.items USING btree (parent_item_id, estado) WHERE (eliminado_en IS NULL);

--
-- Name: outbox_estado_creado_en_idx; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX outbox_estado_creado_en_idx ON item.outbox USING btree (estado, creado_en) WHERE (estado = 'pendiente'::text);

--
-- Name: outbox_estado_creado_en_idx1; Type: INDEX; Schema: item; Owner: -
--

CREATE INDEX outbox_estado_creado_en_idx1 ON item.outbox USING btree (estado, creado_en) WHERE (estado = 'pendiente'::text);

--
-- Name: item_componentes item_componentes_componente_fk; Type: FK CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_componentes
    ADD CONSTRAINT item_componentes_componente_fk FOREIGN KEY (componente_item_id) REFERENCES item.items(id) ON DELETE RESTRICT NOT VALID;

--
-- Name: item_componentes item_componentes_item_fk; Type: FK CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_componentes
    ADD CONSTRAINT item_componentes_item_fk FOREIGN KEY (item_id) REFERENCES item.items(id) ON DELETE CASCADE NOT VALID;

--
-- Name: item_imagenes item_imagenes_item_fk; Type: FK CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_imagenes
    ADD CONSTRAINT item_imagenes_item_fk FOREIGN KEY (item_id) REFERENCES item.items(id) ON DELETE CASCADE NOT VALID;

--
-- Name: item_series item_series_item_fk; Type: FK CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_series
    ADD CONSTRAINT item_series_item_fk FOREIGN KEY (item_id) REFERENCES item.items(id) ON DELETE CASCADE NOT VALID;

--
-- Name: item_series item_series_ubicacion_fk; Type: FK CONSTRAINT; Schema: item; Owner: -
--

ALTER TABLE ONLY item.item_series
    ADD CONSTRAINT item_series_ubicacion_fk FOREIGN KEY (ubicacion_id) REFERENCES inventory.ubicaciones(id) ON DELETE SET NULL NOT VALID;

--
-- Name: historico_precios; Type: ROW SECURITY; Schema: item; Owner: -
--

--
-- Name: historico_precios historico_precios_admin; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: historico_precios historico_precios_empresa; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_atributo_esquemas; Type: ROW SECURITY; Schema: item; Owner: -
--

--
-- Name: item_atributo_esquemas item_atributo_esquemas_admin; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_atributo_esquemas item_atributo_esquemas_empresa; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_componentes; Type: ROW SECURITY; Schema: item; Owner: -
--

--
-- Name: item_componentes item_componentes_admin; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_componentes item_componentes_empresa; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_imagenes; Type: ROW SECURITY; Schema: item; Owner: -
--

--
-- Name: item_imagenes item_imagenes_admin; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_imagenes item_imagenes_empresa; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_series; Type: ROW SECURITY; Schema: item; Owner: -
--

--
-- Name: item_series item_series_admin; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: item_series item_series_empresa; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: items; Type: ROW SECURITY; Schema: item; Owner: -
--

--
-- Name: items items_admin; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: items items_empresa; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: outbox; Type: ROW SECURITY; Schema: item; Owner: -
--

--
-- Name: outbox outbox_admin; Type: POLICY; Schema: item; Owner: -
--

--
-- Name: outbox outbox_app; Type: POLICY; Schema: item; Owner: -
--

--
-- PostgreSQL database dump complete
--
