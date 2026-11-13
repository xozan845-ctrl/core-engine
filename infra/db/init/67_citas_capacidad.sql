-- Fase P1.4 · capacidad en recursos para clases y grupos.
-- El recurso define cuantas citas activas pueden solaparse en el mismo horario.
-- Se reemplaza la exclusion de una sola cita por un trigger que valida el cupo
-- bajo concurrencia (bloquea la fila del recurso).
BEGIN;

ALTER TABLE scheduling.recursos
  ADD COLUMN IF NOT EXISTS capacidad integer NOT NULL DEFAULT 1
  CHECK (capacidad >= 1);

ALTER TABLE scheduling.citas
  DROP CONSTRAINT IF EXISTS citas_recurso_id_periodo_excl;

CREATE INDEX IF NOT EXISTS idx_scheduling_citas_recurso_periodo
  ON scheduling.citas USING gist (recurso_id, periodo);

CREATE OR REPLACE FUNCTION scheduling.validar_cupo_cita() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  v_capacidad integer;
  v_ocupadas  integer;
BEGIN
  IF NEW.estado NOT IN ('reservada', 'confirmada', 'en_atencion') THEN
    RETURN NEW;
  END IF;

  -- Serializa las reservas del mismo recurso: la segunda transaccion espera y
  -- cuenta las filas ya confirmadas por la primera.
  SELECT capacidad INTO v_capacidad
    FROM scheduling.recursos
   WHERE id = NEW.recurso_id
   FOR UPDATE;
  IF v_capacidad IS NULL THEN
    RETURN NEW;
  END IF;

  SELECT COUNT(*)::int INTO v_ocupadas
    FROM scheduling.citas
   WHERE recurso_id = NEW.recurso_id
     AND id <> NEW.id
     AND estado IN ('reservada', 'confirmada', 'en_atencion')
     AND periodo && tstzrange(NEW.inicia_en, NEW.termina_en, '[)');

  IF v_ocupadas >= v_capacidad THEN
    RAISE EXCEPTION 'CUPO_AGOTADO'
      USING ERRCODE = '23P01',
            DETAIL = 'El recurso alcanzo su capacidad para ese horario.';
  END IF;
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_scheduling_cupo_cita ON scheduling.citas;
CREATE TRIGGER trg_scheduling_cupo_cita
  BEFORE INSERT OR UPDATE OF recurso_id, inicia_en, termina_en, estado
  ON scheduling.citas
  FOR EACH ROW
  EXECUTE FUNCTION scheduling.validar_cupo_cita();

COMMENT ON COLUMN scheduling.recursos.capacidad IS
  'Citas activas que pueden solaparse en el mismo horario; 1 = recurso individual.';

COMMIT;
