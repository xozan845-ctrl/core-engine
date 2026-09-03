-- Fase 1: Habilitar PostGIS y migrar tabla de calor
CREATE EXTENSION IF NOT EXISTS postgis;

-- 1. Alterar puntos_calor para agregar la columna geométrica (Point en SRID 4326 que es WGS 84, GPS estandar)
ALTER TABLE intelligence.puntos_calor 
  ADD COLUMN IF NOT EXISTS geom GEOMETRY(Point, 4326);

-- 2. Migrar los datos existentes a la nueva columna
UPDATE intelligence.puntos_calor 
  SET geom = ST_SetSRID(ST_MakePoint(lng, lat), 4326)
  WHERE lat IS NOT NULL AND lng IS NOT NULL AND geom IS NULL;

-- 3. Crear el índice GIST espacial para consultas ultrarrápidas por radio/polígono
CREATE INDEX IF NOT EXISTS idx_intl_calor_geom 
  ON intelligence.puntos_calor USING GIST (geom);

-- 4. Alterar cobertura_zona (Opcional, si quisiéramos cambiar lat_cell/lng_cell por polígonos, 
-- pero por retrocompatibilidad inicial mantendremos las celdas matemáticas + un geom de polígono)
ALTER TABLE intelligence.cobertura_zona
  ADD COLUMN IF NOT EXISTS bounding_box GEOMETRY(Polygon, 4326);

-- (Nota: Para este MVP no llenaremos bounding_box automáticamente hasta tener el job de streaming,
-- pero dejamos la estructura lista).
