-- Core Engine · BodegaHub
-- Esquemas por dominio (Tabla 7). SQL estandar y reproducible en Docker local
-- y en Supabase (AD-08). Los servicios se conectan con un rol de aplicacion.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ── Schemas por dominio ────────────────────────────────────────────────
CREATE SCHEMA IF NOT EXISTS identity;
CREATE SCHEMA IF NOT EXISTS catalog;
CREATE SCHEMA IF NOT EXISTS stores;
CREATE SCHEMA IF NOT EXISTS orders;
CREATE SCHEMA IF NOT EXISTS logistics;
CREATE SCHEMA IF NOT EXISTS commissions;
CREATE SCHEMA IF NOT EXISTS finance;

-- ── identity -------------------------------------------------------------
CREATE TABLE IF NOT EXISTS identity.usuarios (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nombre          text NOT NULL,
  correo          text NOT NULL UNIQUE,
  contrasena_hash text NOT NULL,
  rol             text NOT NULL CHECK (rol IN ('admin', 'vendedor', 'comprador', 'logistica', 'coordinador', 'supervisor', 'operativo')),
  tenant_id       uuid,
  personal_id     uuid,
  creado_en       timestamptz NOT NULL DEFAULT NOW()
);

-- ── catalog --------------------------------------------------------------
CREATE TABLE IF NOT EXISTS catalog.productos (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  sku               text NOT NULL UNIQUE,
  nombre            text NOT NULL,
  descripcion       text NOT NULL DEFAULT '',
  categoria         text NOT NULL DEFAULT 'general',
  precio_base_cents integer NOT NULL CHECK (precio_base_cents >= 0),
  stock             integer NOT NULL DEFAULT 0 CHECK (stock >= 0),
  estado            text NOT NULL DEFAULT 'disponible' CHECK (estado IN ('disponible', 'agotado')),
  creado_en         timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS catalog.historico_precios (
  id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  producto_sku         text NOT NULL REFERENCES catalog.productos(sku) ON DELETE CASCADE,
  precio_anterior_cents integer,
  precio_nuevo_cents   integer NOT NULL,
  vigente_desde        timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS catalog.ajustes_stock (
  id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  producto_sku     text NOT NULL REFERENCES catalog.productos(sku) ON DELETE CASCADE,
  cantidad_anterior integer,
  cantidad_nueva   integer NOT NULL,
  motivo           text NOT NULL,
  realizado_en     timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS catalog.reservas_ordenes (
  order_id   uuid PRIMARY KEY,
  items_json jsonb NOT NULL,
  creado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS catalog.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS catalog.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);
CREATE INDEX IF NOT EXISTS idx_catalog_outbox_pendientes ON catalog.outbox (estado, creado_en)
  WHERE estado = 'pendiente';

-- ── stores ---------------------------------------------------------------
CREATE TABLE IF NOT EXISTS stores.tiendas (
  id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  vendedor_id uuid NOT NULL UNIQUE,
  nombre     text NOT NULL,
  descripcion text NOT NULL DEFAULT '',
  creado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS stores.ofertas (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tienda_id          uuid NOT NULL REFERENCES stores.tiendas(id) ON DELETE CASCADE,
  producto_id        uuid NOT NULL,
  sku                text NOT NULL,
  producto_nombre    text NOT NULL,
  margen             integer NOT NULL CHECK (margen BETWEEN 0 AND 90), -- RN-01: 0-90 %
  precio_base_cents  integer NOT NULL,
  precio_venta_cents integer NOT NULL CHECK (precio_venta_cents >= precio_base_cents),
  stock              integer NOT NULL DEFAULT 0,
  estado             text NOT NULL DEFAULT 'activa' CHECK (estado IN ('activa', 'agotada')),
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (tienda_id, producto_id)
);

CREATE TABLE IF NOT EXISTS stores.historico_precios (
  id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  oferta_id            uuid NOT NULL REFERENCES stores.ofertas(id) ON DELETE CASCADE,
  precio_anterior_cents integer,
  precio_nuevo_cents   integer NOT NULL,
  vigente_desde        timestamptz NOT NULL DEFAULT NOW()
);

-- ── orders (CQRS + Event Sourcing, cap. 3.2) -----------------------------
-- El event store orders.eventos (esquema completo: aggregate_id, aggregate_type,
-- tipo, payload, metadata, version, creado_en) se define en 01d_es_orders.sql
-- para evitar colisión de columnas con el microservicio de pedidos.

CREATE TABLE IF NOT EXISTS orders.proyeccion_ordenes (
  id             uuid PRIMARY KEY,
  cliente_id     uuid NOT NULL,
  items_json     jsonb NOT NULL,
  total_cents    integer NOT NULL,
  estado         text NOT NULL CHECK (estado IN (
                   'creada', 'pagada', 'en_preparacion', 'enviada', 'entregada',
                   'cancelada', 'devuelta')),
  motivo         text,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_orders_proyeccion_cliente ON orders.proyeccion_ordenes (cliente_id, creado_en);

CREATE TABLE IF NOT EXISTS orders.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);
CREATE INDEX IF NOT EXISTS idx_orders_outbox_pendientes ON orders.outbox (estado, creado_en)
  WHERE estado = 'pendiente';

CREATE TABLE IF NOT EXISTS orders.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

-- Carrito del comprador (RN-05): no reserva stock; expira a los 30 minutos
-- de inactividad (actualizado_en + CARRITO_EXPIRACION_MS).
CREATE TABLE IF NOT EXISTS orders.carritos (
  comprador_id   uuid PRIMARY KEY,
  items_json     jsonb NOT NULL DEFAULT '[]',
  total_cents    integer NOT NULL DEFAULT 0,
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_orders_carritos_inactivos ON orders.carritos (actualizado_en);

-- ── logistics -------------------------------------------------------------
CREATE TABLE IF NOT EXISTS logistics.envios (
  id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  guia       text NOT NULL UNIQUE,
  order_id   uuid NOT NULL UNIQUE,
  monto_cents integer NOT NULL,
  estado     text NOT NULL DEFAULT 'en_preparacion' CHECK (estado IN (
               'en_preparacion', 'enviada', 'entregada', 'cancelada')),
  creado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS logistics.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS logistics.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);

-- ── commissions ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS commissions.comisiones (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id            uuid NOT NULL UNIQUE,
  vendedor_id         uuid NOT NULL,
  venta_cents         integer NOT NULL,
  comision_cents      integer NOT NULL,      -- RN-04: 12 % del precio de venta
  monto_vendedor_cents integer NOT NULL,     -- venta - comision
  estado              text NOT NULL DEFAULT 'acreditada' CHECK (estado IN ('acreditada', 'liquidada', 'compensada')),
  creado_en           timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS commissions.pagos (
  id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id   uuid NOT NULL UNIQUE,
  monto_cents integer NOT NULL,
  estado     text NOT NULL DEFAULT 'procesado',
  metodo     text NOT NULL DEFAULT 'simulado', -- MVP: pago simulado
  creado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS commissions.liquidaciones (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  vendedor_id    uuid NOT NULL,
  periodo_inicio date NOT NULL,
  periodo_fin    date NOT NULL,
  monto_cents    integer NOT NULL,
  estado         text NOT NULL DEFAULT 'aprobada' CHECK (estado IN ('aprobada', 'pagada')),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (vendedor_id, periodo_inicio)
);

CREATE TABLE IF NOT EXISTS commissions.compensaciones_devoluciones (
  id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  comision_id uuid REFERENCES commissions.comisiones(id),
  order_id   uuid NOT NULL,
  vendedor_id uuid NOT NULL,
  monto_cents integer NOT NULL,  -- negativo: compensa la comision (RN-06)
  motivo     text NOT NULL,
  creado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS commissions.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS commissions.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);

-- ── identity services table outbox (para consistencia del poller) ---------
CREATE TABLE IF NOT EXISTS identity.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);

CREATE TABLE IF NOT EXISTS stores.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);

-- Idempotencia de eventos consumidos (RN-02: stock.updated sincroniza ofertas).
CREATE TABLE IF NOT EXISTS stores.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

-- ── finance ----------------------------------------------------------------
-- Finanzas, contabilidad y regimen fiscal (cap. 8, 4.4 y 8.11 del informe).
-- Arquitectura multijurisdiccion: cada pais es una fila en finance.jurisdicciones
-- y las tasas/leyes se leen desde ahi, nunca hardcodeadas en el codigo.

-- Plan de cuentas (libros y registros contables conformes a la DGI, Ley 822).
CREATE TABLE IF NOT EXISTS finance.plan_cuentas (
  codigo    text PRIMARY KEY,
  nombre    text NOT NULL,
  tipo      text NOT NULL CHECK (tipo IN ('ACTIVO', 'PASIVO', 'CAPITAL', 'INGRESO', 'COSTO', 'GASTO')),
  naturaleza text NOT NULL CHECK (naturaleza IN ('DEUDORA', 'ACREEDORA')),
  nivel     integer NOT NULL DEFAULT 1,
  estado    text NOT NULL DEFAULT 'activa' CHECK (estado IN ('activa', 'inactiva'))
);

-- Bitacora contable append-only (Event Sourcing contable): un asiento nunca se
-- borra ni se edita; se anula con un asiento inverso (auditoria DGI).
CREATE TABLE IF NOT EXISTS finance.asientos (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  fecha           timestamptz NOT NULL DEFAULT NOW(),
  concepto        text NOT NULL,
  tipo            text NOT NULL DEFAULT 'MANUAL' CHECK (tipo IN ('INGRESO', 'EGRESO', 'AJUSTE', 'CIERRE', 'APERTURA', 'MANUAL')),
  referencia_tipo text,
  referencia_id   text,
  moneda          text NOT NULL DEFAULT 'C$',
  estado          text NOT NULL DEFAULT 'REGISTRADO' CHECK (estado IN ('REGISTRADO', 'ANULADO')),
  creado_por      text,
  creado_en       timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS finance.asiento_detalles (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  asiento_id  uuid NOT NULL REFERENCES finance.asientos(id) ON DELETE CASCADE,
  cuenta_codigo text NOT NULL REFERENCES finance.plan_cuentas(codigo),
  debe_cents  integer NOT NULL DEFAULT 0 CHECK (debe_cents >= 0),
  haber_cents integer NOT NULL DEFAULT 0 CHECK (haber_cents >= 0),
  concepto    text,
  orden       integer NOT NULL DEFAULT 0,
  CHECK (NOT (debe_cents = 0 AND haber_cents = 0))
);
CREATE INDEX IF NOT EXISTS idx_finance_detalles_asiento ON finance.asiento_detalles (asiento_id);
CREATE INDEX IF NOT EXISTS idx_finance_detalles_cuenta ON finance.asiento_detalles (cuenta_codigo);

-- CQRS logico (ADR-07): el lado de lectura se materializa en proyecciones
-- dedicadas alimentadas por la bitacora append-only de asientos/detalles, de
-- modo que los queries NUNCA hacen JOINs sobre las tablas de escritura. Los
-- triggers mantienen consistencia en la misma transaccion (rollback preserving)
-- e incluyen el estado 'ANULADO' (libro mayor DGI filtra REGISTRADOS; libro
-- diario incluye todo tal cual el append-only).
CREATE TABLE IF NOT EXISTS finance.asientos_vista (
  id              uuid PRIMARY KEY,              -- id_asiento (FK implicita)
  fecha           timestamptz NOT NULL,
  concepto        text NOT NULL,
  tipo            text NOT NULL,
  referencia_tipo text,
  referencia_id   text,
  moneda          text NOT NULL,
  estado          text NOT NULL,
  creado_por      text,
  creado_en       timestamptz NOT NULL,
  debe_cents      integer NOT NULL,
  haber_cents     integer NOT NULL,
  periodo         date NOT NULL,               -- primer dia del mes (YYYY-MM-01)
  detalles_jsonb  jsonb NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_asientos_vista_periodo ON finance.asientos_vista (periodo);
CREATE INDEX IF NOT EXISTS idx_asientos_vista_estado ON finance.asientos_vista (estado);

-- Read model del libro mayor: saldo acumulado por cuenta+mes consolidado desde
-- la historia de movimientos (append-only). Mantiene movimientos_jsonb para
-- el detalle del libro mayor. Solo movimientos de asientos REGISTRADOS.
CREATE TABLE IF NOT EXISTS finance.libro_mayor_mv (
  cuenta_codigo  text NOT NULL,
  periodo        date NOT NULL,               -- primer dia del mes
  debe_cents     integer NOT NULL DEFAULT 0,
  haber_cents    integer NOT NULL DEFAULT 0,
  saldo_cents    integer NOT NULL DEFAULT 0,
  movimientos_jsonb jsonb NOT NULL DEFAULT '[]',
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (cuenta_codigo, periodo)
);
CREATE INDEX IF NOT EXISTS idx_libro_mayor_cuenta ON finance.libro_mayor_mv (cuenta_codigo);

-- Funcion de proyeccion CQRS (ADR-07): reconstruye from scratch las vistas
-- asientos_vista y libro_mayor_mv para un mes (idempotente, pues la bitacora
-- de asientos es append-only). Disparada por trigger para el mes afectado; es
-- segura bajo rollback (solo actua al commit) y libre de drift de acumulados.
CREATE OR REPLACE FUNCTION finance.fn_refrescar_mes(p_mes date)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  -- 1) asientos_vista para todo el mes
  DELETE FROM finance.asientos_vista WHERE periodo = p_mes;
  INSERT INTO finance.asientos_vista
    (id, fecha, concepto, tipo, referencia_tipo, referencia_id, moneda, estado,
     creado_por, creado_en, debe_cents, haber_cents, periodo, detalles_jsonb)
  SELECT a.id, a.fecha, a.concepto, a.tipo, a.referencia_tipo, a.referencia_id,
         a.moneda, a.estado, a.creado_por, a.creado_en,
         COALESCE(d.debe_cents, 0), COALESCE(d.haber_cents, 0),
         date_trunc('month', a.fecha)::date,
         COALESCE(d.aggs, '[]'::jsonb)
  FROM finance.asientos a
  LEFT JOIN (
    SELECT asiento_id,
           SUM(debe_cents) AS debe_cents, SUM(haber_cents) AS haber_cents,
           jsonb_agg(jsonb_build_object(
             'cuenta_codigo', cuenta_codigo, 'debe_cents', debe_cents,
             'haber_cents', haber_cents, 'concepto', concepto, 'orden', orden
           ) ORDER BY orden) AS aggs
    FROM finance.asiento_detalles d
    JOIN finance.asientos a2 ON a2.id = d.asiento_id
    WHERE date_trunc('month', a2.fecha)::date = p_mes
    GROUP BY asiento_id
  ) d ON d.asiento_id = a.id
  WHERE date_trunc('month', a.fecha)::date = p_mes;

  -- 2) libro_mayor_mv para todo el mes: movimiento consolidado por cuenta+mes
  --    (saldo acumulado del mes = debe - haber segun naturaleza). El reader
  --    calcula el saldo inicial con la acumulacion de meses previos.
  DELETE FROM finance.libro_mayor_mv WHERE periodo = p_mes;
  INSERT INTO finance.libro_mayor_mv
    (cuenta_codigo, periodo, debe_cents, haber_cents, saldo_cents, movimientos_jsonb, actualizado_en)
  SELECT d.cuenta_codigo,
         p_mes AS periodo,
         SUM(d.debe_cents) AS debe_cents,
         SUM(d.haber_cents) AS haber_cents,
         CASE WHEN MAX(c.naturaleza) = 'DEUDORA'
              THEN SUM(d.debe_cents) - SUM(d.haber_cents)
              ELSE SUM(d.haber_cents) - SUM(d.debe_cents) END AS saldo_cents,
         COALESCE(jsonb_agg(d.mov ORDER BY d.fecha, d.asiento_id), '[]'::jsonb) AS movimientos_jsonb,
         NOW()
  FROM (
    SELECT a.id AS asiento_id, a.fecha, d.cuenta_codigo, d.debe_cents, d.haber_cents,
           jsonb_build_object(
             'asiento_id', a.id, 'fecha', a.fecha, 'concepto', d.concepto,
             'debe_cents', d.debe_cents, 'haber_cents', d.haber_cents, 'orden', d.orden
           ) AS mov
    FROM finance.asiento_detalles d
    JOIN finance.asientos a ON a.id = d.asiento_id
    WHERE a.estado = 'REGISTRADO' AND date_trunc('month', a.fecha)::date = p_mes
  ) d
  JOIN finance.plan_cuentas c ON c.codigo = d.cuenta_codigo
  GROUP BY d.cuenta_codigo;
END;
$$;

-- Triggers de proyeccion (AFTER: actua sobre el mes del asiento afectado)
CREATE OR REPLACE FUNCTION finance.fn_trigger_asiento() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  mesNuevo date := date_trunc('month', COALESCE(NEW.fecha, NOW()))::date;
  mesViejo date;
BEGIN
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    -- refrescar el mes del asiento (y el viejo si cambio de mes)
    PERFORM finance.fn_refrescar_mes(mesNuevo);
    IF TG_OP = 'UPDATE' THEN
      mesViejo := date_trunc('month', OLD.fecha)::date;
      IF mesViejo IS DISTINCT FROM mesNuevo THEN
        PERFORM finance.fn_refrescar_mes(mesViejo);
      END IF;
    END IF;
  END IF;
  RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION finance.fn_trigger_detalle() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  mes date;
BEGIN
  SELECT date_trunc('month', a.fecha)::date INTO mes
  FROM finance.asientos a WHERE a.id = COALESCE(NEW.asiento_id, OLD.asiento_id);
  IF FOUND THEN
    PERFORM finance.fn_refrescar_mes(mes);
  END IF;
  RETURN NULL;
END;
$$;

-- Triggers de proyeccion (solo para la migracion de datos historicos existentes;
-- en fresh DB no hay asientos aun, por lo que el DO no refresca nada)
CREATE OR REPLACE TRIGGER asientos_proyeccion_trigger
AFTER INSERT OR UPDATE ON finance.asientos
FOR EACH ROW EXECUTE FUNCTION finance.fn_trigger_asiento();

CREATE OR REPLACE TRIGGER detalles_proyeccion_trigger
AFTER INSERT OR UPDATE OR DELETE ON finance.asiento_detalles
FOR EACH ROW EXECUTE FUNCTION finance.fn_trigger_detalle();

-- Bootstrap idempotente: materializa los meses ya existentes en la bitacora
CREATE OR REPLACE FUNCTION finance.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  mes DATE;
BEGIN
  FOR mes IN
    SELECT DISTINCT date_trunc('month', fecha)::date FROM finance.asientos
  LOOP
    PERFORM finance.fn_refrescar_mes(mes);
  END LOOP;
END;
$$;
SELECT finance.fn_bootstrap_proyecciones();

-- Comprobantes fiscales DGI (cap. 4.4): series con prefijos y secuenciales.
CREATE TABLE IF NOT EXISTS finance.series_comprobantes (
  serie           text PRIMARY KEY,
  tipo            text NOT NULL CHECK (tipo IN ('FACTURA', 'NOTA_CREDITO')),
  prefijo         text NOT NULL,
  secuencial_actual integer NOT NULL DEFAULT 0,
  ruc_emisor      text NOT NULL,
  jurisdiccion    text NOT NULL DEFAULT 'NI',
  activa          boolean NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS finance.comprobantes (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  serie              text NOT NULL REFERENCES finance.series_comprobantes(serie),
  numero             text NOT NULL,
  tipo               text NOT NULL CHECK (tipo IN ('FACTURA', 'NOTA_CREDITO')),
  orden_id           uuid,
  cliente_id         uuid,
  base_gravada_cents integer NOT NULL,
  iva_cents          integer NOT NULL DEFAULT 0,   -- Ley 822: IVA 15 %
  exento_cents       integer NOT NULL DEFAULT 0,
  total_cents        integer NOT NULL,
  moneda             text NOT NULL DEFAULT 'C$',   -- Ley 842: facturacion en cordobas
  estado             text NOT NULL DEFAULT 'BORRADOR' CHECK (estado IN ('BORRADOR', 'EMITIDO', 'ANULADO')),
  datos_cliente      jsonb,
  emitido_en         timestamptz,
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (serie, numero)
);

-- Multijurisdiccion: un pais por fila; tasa y marco legal configurables.
CREATE TABLE IF NOT EXISTS finance.jurisdicciones (
  codigo_pais        text PRIMARY KEY,
  nombre             text NOT NULL,
  moneda             text NOT NULL,
  simbolo_moneda     text NOT NULL,
  tasa_iva           numeric(5, 4) NOT NULL,       -- NI: 0.1500 (Ley 822)
  tasa_ir            numeric(5, 4) NOT NULL,       -- tasa general de referencia
  periodicidad_declaracion text NOT NULL DEFAULT 'MENSUAL',
  leyes              jsonb NOT NULL DEFAULT '{}',
  activa             boolean NOT NULL DEFAULT FALSE
);

-- Regimen fiscal de los vendedores (Régimen de Cuota Fija o Régimen General).
CREATE TABLE IF NOT EXISTS finance.regimenes_fiscales (
  id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  jurisdiccion         text NOT NULL REFERENCES finance.jurisdicciones(codigo_pais),
  codigo               text NOT NULL,
  nombre               text NOT NULL,
  descripcion          text NOT NULL DEFAULT '',
  periodicidad         text NOT NULL DEFAULT 'MENSUAL',
  condicion_ingresos_anuales_cents integer,  -- limite aproximado; la DGI es la fuente oficial
  estado               text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'inactivo')),
  UNIQUE (jurisdiccion, codigo)
);

CREATE TABLE IF NOT EXISTS finance.sujetos_fiscales (
  id            uuid PRIMARY KEY,   -- mismo id que identity.usuarios (vendedor/admin)
  jurisdiccion  text NOT NULL REFERENCES finance.jurisdicciones(codigo_pais),
  regimen_id    uuid REFERENCES finance.regimenes_fiscales(id),
  razon_social  text NOT NULL,
  ruc           text,
  es_plataforma boolean NOT NULL DEFAULT FALSE,
  estado        text NOT NULL DEFAULT 'ACTIVO' CHECK (estado IN ('ACTIVO', 'BAJA')),
  creado_en     timestamptz NOT NULL DEFAULT NOW()
);

-- Declaraciones fiscales periodicas (IR e IVA mensual, doc 4.4).
CREATE TABLE IF NOT EXISTS finance.declaraciones (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  jurisdiccion          text NOT NULL REFERENCES finance.jurisdicciones(codigo_pais),
  tipo                  text NOT NULL CHECK (tipo IN ('IVA', 'IR', 'CUOTA_FIJA')),
  periodo_inicio        date NOT NULL,
  periodo_fin           date NOT NULL,
  base_gravada_cents    integer NOT NULL DEFAULT 0,
  iva_debitado_cents    integer NOT NULL DEFAULT 0,
  iva_credito_cents     integer NOT NULL DEFAULT 0,
  iva_a_pagar_cents     integer NOT NULL DEFAULT 0,
  renta_bruta_cents     integer NOT NULL DEFAULT 0,
  renta_gravable_cents  integer NOT NULL DEFAULT 0,
  ir_a_pagar_cents      integer NOT NULL DEFAULT 0,
  cuota_cents           integer NOT NULL DEFAULT 0,
  estado                text NOT NULL DEFAULT 'PENDIENTE' CHECK (estado IN ('PENDIENTE', 'GENERADA', 'PRESENTADA', 'PAGADA', 'CANCELADA')),
  detalle               jsonb,
  generada_en           timestamptz,
  presentada_en         timestamptz,
  creado_en             timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (jurisdiccion, tipo, periodo_inicio)
);

-- Modelo de proyeccion financiera (cap. 8.6): formulas auditables, filas mensuales.
CREATE TABLE IF NOT EXISTS finance.proyecciones (
  id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nombre               text NOT NULL,
  horizonte_meses      integer NOT NULL,
  supuestos            jsonb NOT NULL,
  filas                jsonb NOT NULL,
  totales              jsonb NOT NULL,
  inversion_inicial_cents integer NOT NULL DEFAULT 0,
  creado_en            timestamptz NOT NULL DEFAULT NOW()
);

-- Proyeccion CQRS de metricas mensuales alimentada por eventos del bus (cap. 8.8).
CREATE TABLE IF NOT EXISTS finance.metricas_mensuales (
  periodo      date NOT NULL,          -- primer dia del mes
  tipo         text NOT NULL,          -- gmv | ingresos_comisiones | pedidos | devoluciones
  monto_cents  integer NOT NULL DEFAULT 0,
  cantidad     integer NOT NULL DEFAULT 0,
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (periodo, tipo)
);

CREATE TABLE IF NOT EXISTS finance.metricas_vendedores (
  vendedor_id      uuid NOT NULL,
  periodo          date NOT NULL,      -- mes de la primera venta registrada
  ventas           integer NOT NULL DEFAULT 0,
  monto_cents      integer NOT NULL DEFAULT 0,
  comision_cents   integer NOT NULL DEFAULT 0,
  devoluciones     integer NOT NULL DEFAULT 0,
  primera_venta_en timestamptz,
  ultima_venta_en  timestamptz,
  actualizado_en   timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (vendedor_id, periodo)
);

CREATE TABLE IF NOT EXISTS finance.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS finance.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente' CHECK (estado IN ('pendiente', 'publicado')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);
CREATE INDEX IF NOT EXISTS idx_finance_outbox_pendientes ON finance.outbox (estado, creado_en)
  WHERE estado = 'pendiente';

-- ── Seeds del dominio finance (Nicaragua inicial) --------------------------
-- Jurisdiccion inicial: Nicaragua (Ley 822: IVA 15 %; Ley 842: C$; Ley 787: datos).
INSERT INTO finance.jurisdicciones
  (codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir, periodicidad_declaracion, leyes, activa)
VALUES
  ('NI', 'Nicaragua', 'C$', 'C$', 0.1500, 0.3000, 'MENSUAL',
   '{"iva": "Ley 822 (Ley de Concertacion Tributaria)", "consumidor": "Ley 842 (Proteccion de los Derechos de las Personas Consumidoras y Usuarias)", "datos": "Ley 787 (Proteccion de Datos Personales)"}', TRUE)
ON CONFLICT (codigo_pais) DO NOTHING;

-- Regimenes de la DGI para vendedores segun su nivel de ingresos (cap. 4.4).
INSERT INTO finance.regimenes_fiscales
  (jurisdiccion, codigo, nombre, descripcion, periodicidad, condicion_ingresos_anuales_cents)
VALUES
  ('NI', 'CUOTA_FIJA', 'Regimen de Cuota Fija',
   'Regimen simplificado de la DGI para pequenos contribuyentes; cuota mensual fija determinada por la DGI.', 'MENSUAL', 100000000),
  ('NI', 'REGIMEN_GENERAL', 'Regimen General',
   'Declaracion mensual de IR e IVA (15 %) conforme a la Ley 822.', 'MENSUAL', NULL)
ON CONFLICT (jurisdiccion, codigo) DO NOTHING;

-- Plan de cuentas de referencia para la plataforma (empresa de servicios).
INSERT INTO finance.plan_cuentas (codigo, nombre, tipo, naturaleza, nivel) VALUES
  ('1', 'ACTIVO', 'ACTIVO', 'DEUDORA', 1),
  ('1.1', 'ACTIVO CORRIENTE', 'ACTIVO', 'DEUDORA', 2),
  ('1.1.1', 'Caja', 'ACTIVO', 'DEUDORA', 3),
  ('1.1.2', 'Bancos', 'ACTIVO', 'DEUDORA', 3),
  ('1.1.3', 'Cuentas por cobrar (compradores)', 'ACTIVO', 'DEUDORA', 3),
  ('1.1.4', 'IVA por cobrar', 'ACTIVO', 'DEUDORA', 3),
  ('1.2', 'ACTIVO NO CORRIENTE', 'ACTIVO', 'DEUDORA', 2),
  ('2', 'PASIVO', 'PASIVO', 'ACREEDORA', 1),
  ('2.1', 'PASIVO CORRIENTE', 'PASIVO', 'ACREEDORA', 2),
  ('2.1.1', 'Acreedores vendedores (comisiones por liquidar)', 'PASIVO', 'ACREEDORA', 3),
  ('2.1.2', 'Fondos por liquidar', 'PASIVO', 'ACREEDORA', 3),
  ('2.1.3', 'IVA por pagar', 'PASIVO', 'ACREEDORA', 3),
  ('2.1.4', 'Retenciones por pagar', 'PASIVO', 'ACREEDORA', 3),
  ('2.1.5', 'Ingresos diferidos', 'PASIVO', 'ACREEDORA', 3),
  ('3', 'CAPITAL', 'CAPITAL', 'ACREEDORA', 1),
  ('3.1', 'Capital social', 'CAPITAL', 'ACREEDORA', 2),
  ('3.2', 'Resultados acumulados', 'CAPITAL', 'ACREEDORA', 2),
  ('4', 'INGRESOS', 'INGRESO', 'ACREEDORA', 1),
  ('4.1', 'Ingresos por comisiones', 'INGRESO', 'ACREEDORA', 2),
  ('4.2', 'Ingresos por servicios premium', 'INGRESO', 'ACREEDORA', 2),
  ('5', 'COSTOS', 'COSTO', 'DEUDORA', 1),
  ('5.1', 'Costo de operacion', 'COSTO', 'DEUDORA', 2),
  ('6', 'GASTOS', 'GASTO', 'DEUDORA', 1),
  ('6.1', 'Gastos operativos', 'GASTO', 'DEUDORA', 2),
  ('6.2', 'Gastos por impuestos', 'GASTO', 'DEUDORA', 2)
ON CONFLICT (codigo) DO NOTHING;

-- Serie de facturacion inicial (solo aplica al activar EMITIR_COMPROBANTES_FISCALES=true).
INSERT INTO finance.series_comprobantes (serie, tipo, prefijo, secuencial_actual, ruc_emisor) VALUES
  ('F001', 'FACTURA', 'F001-', 0, 'RUC-PLATAFORMA-0001'),
  ('NC001', 'NOTA_CREDITO', 'NC001-', 0, 'RUC-PLATAFORMA-0001')
ON CONFLICT (serie) DO NOTHING;