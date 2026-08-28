-- CQRS logico (ADR-07): proyecciones de lectura para finance. Idempotente
-- (CREATE TABLE/FUNCTION IF [OR REPLACE NOT] EXISTS, triggers IF NOT EXISTS).
-- Se ejecuta despues de 01_esquemas (ord alfabetico) tanto en fresh like en DB viva.
CREATE SCHEMA IF NOT EXISTS finance;

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

CREATE TABLE IF NOT EXISTS finance.libro_mayor_mv (
  cuenta_codigo  text NOT NULL,
  periodo        date NOT NULL,
  debe_cents     integer NOT NULL DEFAULT 0,
  haber_cents    integer NOT NULL DEFAULT 0,
  saldo_cents    integer NOT NULL DEFAULT 0,
  movimientos_jsonb jsonb NOT NULL DEFAULT '[]',
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (cuenta_codigo, periodo)
);
CREATE INDEX IF NOT EXISTS idx_libro_mayor_cuenta ON finance.libro_mayor_mv (cuenta_codigo);

CREATE OR REPLACE FUNCTION finance.fn_refrescar_mes(p_mes date)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  -- Serializa refrescos concurrentes del mismo mes (race DELETE+INSERT → PK duplicado)
  PERFORM pg_advisory_xact_lock(hashtext('finance.asientos_vista:' || p_mes::text));
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
             'haber_cents', haber_cents, 'concepto', d.concepto, 'orden', orden
           ) ORDER BY orden) AS aggs
    FROM finance.asiento_detalles d
    JOIN finance.asientos a2 ON a2.id = d.asiento_id
    WHERE date_trunc('month', a2.fecha)::date = p_mes
    GROUP BY asiento_id
  ) d ON d.asiento_id = a.id
  WHERE date_trunc('month', a.fecha)::date = p_mes;

  DELETE FROM finance.libro_mayor_mv WHERE periodo = p_mes;
  INSERT INTO finance.libro_mayor_mv
    (cuenta_codigo, periodo, debe_cents, haber_cents, saldo_cents, movimientos_jsonb, actualizado_en)
  SELECT c.codigo AS cuenta_codigo,
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
  GROUP BY c.codigo;
END;
$$;

CREATE OR REPLACE FUNCTION finance.fn_trigger_asiento() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
  mesNuevo date := date_trunc('month', COALESCE(NEW.fecha, NOW()))::date;
  mesViejo date;
BEGIN
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
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

CREATE OR REPLACE FUNCTION finance.fn_trigger_detalle() RETURNS TRIGGER LANGUAGE plpgsql AS $$
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

DROP TRIGGER IF EXISTS asientos_proyeccion_trigger ON finance.asientos;
CREATE TRIGGER asientos_proyeccion_trigger
AFTER INSERT OR UPDATE ON finance.asientos
FOR EACH ROW EXECUTE FUNCTION finance.fn_trigger_asiento();

DROP TRIGGER IF EXISTS detalles_proyeccion_trigger ON finance.asiento_detalles;
CREATE TRIGGER detalles_proyeccion_trigger
AFTER INSERT OR UPDATE OR DELETE ON finance.asiento_detalles
FOR EACH ROW EXECUTE FUNCTION finance.fn_trigger_detalle();

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
