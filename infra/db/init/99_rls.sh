#!/bin/sh
# Aplica las políticas RLS canónicas de Supabase también en el Postgres local
# (R-SB-2). Se ejecuta después de los esquemas y antes de 99z_grants.sql.
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" \
  -f /rls/99_rls.sql
