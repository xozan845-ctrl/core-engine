-- Core Engine - market-intelligence-service
-- Schema intelligence: inteligencia de mercado y analisis geoespacial.
-- Coordenadas GPS completas para precision en mapas de calor.
-- Datos demograficos en rangos anonimos (ciencias de datos, no PII exacta).

CREATE SCHEMA IF NOT EXISTS intelligence;

-- ── hechos_venta (evento atomico de venta con contexto geo) ───────────────
CREATE TABLE IF NOT EXISTS intelligence.hechos_venta (
  id                        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id                  uuid NOT NULL,
  sku                       text NOT NULL,
  vendedor_id               uuid NOT NULL,
  cantidad                  integer NOT NULL DEFAULT 1,
  monto_cents               bigint NOT NULL,
  lat                       double precision,
  lng                       double precision,
  gps_precision_metros      double precision,
  gps_velocidad_ms          double precision,
  gps_rumbo_grados          double precision,
  tipo_actividad            text CHECK (tipo_actividad IN ('impulsacion', 'venta', 'entrega', 'reparto')),
  resultado_visita          text CHECK (resultado_visita IN ('visitado', 'cerca_no_visitado', 'no_visitado')),
  distancia_cliente_metros  double precision,
  rango_edad                text CHECK (rango_edad IN ('18-24', '25-34', '35-44', '45-54', '55+')),
  genero                    text CHECK (genero IN ('M', 'F', 'NS')),
  ocurrido_en               timestamptz NOT NULL,
  creado_en                 timestamptz NOT NULL DEFAULT NOW(),
  deleted_at                timestamptz,
  UNIQUE (order_id, sku)
);
CREATE INDEX IF NOT EXISTS idx_intl_hv_vendedor  ON intelligence.hechos_venta (vendedor_id);
CREATE INDEX IF NOT EXISTS idx_intl_hv_sku       ON intelligence.hechos_venta (sku);
CREATE INDEX IF NOT EXISTS idx_intl_hv_geo       ON intelligence.hechos_venta (lat, lng)
  WHERE lat IS NOT NULL AND lng IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_intl_hv_tiempo    ON intelligence.hechos_venta (ocurrido_en DESC);
CREATE INDEX IF NOT EXISTS idx_intl_hv_actividad ON intelligence.hechos_venta (tipo_actividad);

-- ── rendimiento_vendedor (vista materializada por evento) ────────────────
CREATE TABLE IF NOT EXISTS intelligence.rendimiento_vendedor (
  vendedor_id               uuid PRIMARY KEY,
  total_ventas              integer NOT NULL DEFAULT 0,
  total_monto_cents         bigint NOT NULL DEFAULT 0,
  ticket_promedio_cents     bigint NOT NULL DEFAULT 0,
  pedidos_entregados        integer NOT NULL DEFAULT 0,
  pedidos_fallidos          integer NOT NULL DEFAULT 0,
  tasa_efectividad          numeric(5,2) NOT NULL DEFAULT 0,
  rutas_completadas         integer NOT NULL DEFAULT 0,
  dwell_time_promedio_min   numeric(8,2) NOT NULL DEFAULT 0,
  cobertura_clientes        numeric(5,2) NOT NULL DEFAULT 0,
  actualizado_en            timestamptz NOT NULL DEFAULT NOW()
);

-- ── cobertura_zona (densidad geografica por celda 0.01 grados ~ 1.1 km) ──
CREATE TABLE IF NOT EXISTS intelligence.cobertura_zona (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  lat_cell            double precision NOT NULL,
  lng_cell            double precision NOT NULL,
  total_ventas        integer NOT NULL DEFAULT 0,
  total_monto_cents   bigint NOT NULL DEFAULT 0,
  total_vendedores    integer NOT NULL DEFAULT 0,
  skus_top            jsonb NOT NULL DEFAULT '[]',
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (lat_cell, lng_cell)
);
CREATE INDEX IF NOT EXISTS idx_intl_zona_grid ON intelligence.cobertura_zona (lat_cell, lng_cell);

-- ── metricas_producto (demanda, rotacion, zonas activas) ─────────────────
CREATE TABLE IF NOT EXISTS intelligence.metricas_producto (
  sku                 text PRIMARY KEY,
  nombre              text NOT NULL DEFAULT '',
  total_vendido       integer NOT NULL DEFAULT 0,
  total_monto_cents   bigint NOT NULL DEFAULT 0,
  stock_actual        integer NOT NULL DEFAULT 0,
  ultima_venta_en     timestamptz,
  zonas_activas       jsonb NOT NULL DEFAULT '[]',
  actualizado_en      timestamptz NOT NULL DEFAULT NOW()
);

-- ── puntos_calor (alta resolucion, un punto por hecho de venta/visita) ────
CREATE TABLE IF NOT EXISTS intelligence.puntos_calor (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  lat         double precision NOT NULL,
  lng         double precision NOT NULL,
  peso        integer NOT NULL DEFAULT 1,
  sku         text,
  vendedor_id uuid,
  tipo        text NOT NULL DEFAULT 'venta',
  ocurrido_en timestamptz NOT NULL,
  deleted_at  timestamptz
);
CREATE INDEX IF NOT EXISTS idx_intl_calor_geo      ON intelligence.puntos_calor (lat, lng);
CREATE INDEX IF NOT EXISTS idx_intl_calor_tiempo   ON intelligence.puntos_calor (ocurrido_en DESC);
CREATE INDEX IF NOT EXISTS idx_intl_calor_tipo     ON intelligence.puntos_calor (tipo);
CREATE INDEX IF NOT EXISTS idx_intl_calor_vendedor ON intelligence.puntos_calor (vendedor_id);

-- ── tendencias_temporales (agregado por dia, sku y vendedor) ─────────────
CREATE TABLE IF NOT EXISTS intelligence.tendencias_temporales (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  fecha             date NOT NULL,
  sku               text,
  vendedor_id       uuid,
  total_ventas      integer NOT NULL DEFAULT 0,
  total_monto_cents bigint NOT NULL DEFAULT 0,
  dia_semana        integer NOT NULL DEFAULT 0,
  UNIQUE (fecha, sku, vendedor_id)
);
CREATE INDEX IF NOT EXISTS idx_intl_tend_fecha    ON intelligence.tendencias_temporales (fecha DESC);
CREATE INDEX IF NOT EXISTS idx_intl_tend_sku      ON intelligence.tendencias_temporales (sku);
CREATE INDEX IF NOT EXISTS idx_intl_tend_vendedor ON intelligence.tendencias_temporales (vendedor_id);

-- ── eventos_procesados (idempotencia, data-integrity.md regla 4) ──────────
CREATE TABLE IF NOT EXISTS intelligence.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

-- ── outbox (patron ADR-03, architecture.md regla 3) ──────────────────────
CREATE TABLE IF NOT EXISTS intelligence.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);
CREATE INDEX IF NOT EXISTS idx_intl_outbox_pendientes ON intelligence.outbox (estado, creado_en)
  WHERE estado = 'pendiente';
