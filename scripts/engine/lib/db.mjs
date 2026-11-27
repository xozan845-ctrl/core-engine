/**
 * Integridad de las migraciones SQL versionadas (R-DB-6).
 *
 * `infra/db/init/*.sql` se aplica al arrancar el contenedor y
 * `infra/db/supabase/migrations/*.sql` contiene los cambios incrementales de
 * RLS aplicados después del endurecimiento. Ninguno tiene historial por sí
 * mismo: para que una migración ya aplicada no se edite en silencio,
 * `scripts/config/db-migrations.json` guarda el SHA-256 de cada archivo y la
 * suite de reglas falla si un hash cambia, si falta un archivo o si aparece uno
 * nuevo sin registrar.
 */
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

export const MIGRATIONS_DIR = 'infra/db/init';
export const MIGRATION_PATTERN = /^\d{2}[a-z]?_.+\.sql$/;
export const RLS_MIGRATIONS_DIR = 'infra/db/supabase/migrations';
export const RLS_MIGRATION_PATTERN = /^\d{8}_\d{2}_[a-z0-9_]+\.sql$/;
export const RLS_FILE = 'infra/db/supabase/99_rls.sql';

export function migrationsDirectory(root) {
  return join(root, MIGRATIONS_DIR);
}

function listSqlFilesIn(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.sql'))
    .map((entry) => entry.name)
    .sort();
}

/** Lista todos los `.sql` del directorio de init, válidos o no. */
export function listSqlFiles(root) {
  const directory = migrationsDirectory(root);
  if (!existsSync(directory)) {
    throw new Error(`No existe ${directory}.`);
  }
  return listSqlFilesIn(directory);
}

/** Lista solo las migraciones init con el nombre canónico `NN[tema].sql`. */
export function listMigrationFiles(root) {
  return listSqlFiles(root).filter((name) => MIGRATION_PATTERN.test(name));
}

/** Lista las migraciones incrementales de RLS. */
export function listRlsMigrationFiles(root) {
  return listSqlFilesIn(join(root, RLS_MIGRATIONS_DIR)).filter((name) =>
    RLS_MIGRATION_PATTERN.test(name),
  );
}

/** Directorios versionados con su patrón de nombre canónico. */
function migrationSources(root) {
  return [
    {
      dir: join(root, MIGRATIONS_DIR),
      pattern: MIGRATION_PATTERN,
      hint: 'NN_tema.sql',
    },
    {
      dir: join(root, RLS_MIGRATIONS_DIR),
      pattern: RLS_MIGRATION_PATTERN,
      hint: 'AAAAMMDD_NN_tema.sql',
    },
  ];
}

export function sha256File(file) {
  return createHash('sha256').update(readFileSync(file)).digest('hex');
}

/** Manifiesto `archivo -> sha256` de todas las migraciones válidas. */
export function buildMigrationManifest(root) {
  const manifest = {};
  for (const { dir, pattern } of migrationSources(root)) {
    for (const name of listSqlFilesIn(dir)) {
      if (!pattern.test(name)) continue;
      manifest[name] = sha256File(join(dir, name));
    }
  }
  return manifest;
}

/**
 * Compara los directorios reales con el manifiesto versionado.
 *
 * @param {string} root
 * @param {Record<string, string>} manifest
 * @returns {string[]}
 */
export function checkMigrationManifest(root, manifest) {
  const problems = [];
  const valid = new Set();

  for (const { dir, pattern, hint } of migrationSources(root)) {
    for (const name of listSqlFilesIn(dir)) {
      if (!pattern.test(name)) {
        problems.push(`${name}: nombre inválido; usa ${hint} (R-DB-6)`);
        continue;
      }
      if (valid.has(name)) {
        problems.push(`${name}: nombre duplicado entre directorios de migraciones`);
        continue;
      }
      valid.add(name);

      if (!(name in manifest)) {
        problems.push(
          `${name}: migración nueva sin registrar en scripts/config/db-migrations.json`,
        );
        continue;
      }

      if (sha256File(join(dir, name)) !== manifest[name]) {
        problems.push(
          `${name}: hash distinto al registrado; una migración aplicada no se edita, crea una nueva`,
        );
      }
    }
  }

  for (const name of Object.keys(manifest)) {
    if (!valid.has(name)) {
      problems.push(`${name}: registrada en el manifiesto pero ya no existe en el repositorio`);
    }
  }

  return problems;
}

/**
 * Estado de RLS por tabla creada en `infra/db/init/*.sql`.
 *
 * @param {string} root
 * @returns {{ table: string, rls: boolean, policies: number }[]}
 */
export function readRlsStatus(root) {
  const tables = new Set();
  for (const name of listMigrationFiles(root)) {
    const sql = readFileSync(join(migrationsDirectory(root), name), 'utf8');
    for (const match of sql.matchAll(
      /CREATE TABLE\s+(?:IF NOT EXISTS\s+)?([a-z_][a-z0-9_]*)\.([a-z_][a-z0-9_]*)/gi,
    )) {
      tables.add(`${match[1].toLowerCase()}.${match[2].toLowerCase()}`);
    }
    for (const match of sql.matchAll(
      /DROP TABLE\s+(?:IF EXISTS\s+)?([a-z_][a-z0-9_]*)\.([a-z_][a-z0-9_]*)/gi,
    )) {
      tables.delete(`${match[1].toLowerCase()}.${match[2].toLowerCase()}`);
    }
    for (const match of sql.matchAll(
      /DROP SCHEMA\s+(?:IF EXISTS\s+)?([a-z_][a-z0-9_]*)/gi,
    )) {
      const schema = `${match[1].toLowerCase()}.`;
      for (const table of [...tables]) {
        if (table.startsWith(schema)) tables.delete(table);
      }
    }
  }

  const rlsPath = join(root, RLS_FILE);
  const rlsSql = existsSync(rlsPath) ? readFileSync(rlsPath, 'utf8') : '';
  const enabled = new Set();
  const policies = new Map();

  for (const match of rlsSql.matchAll(
    /ALTER TABLE\s+([a-z_][a-z0-9_]*)\.([a-z_][a-z0-9_]*)\s+ENABLE ROW LEVEL SECURITY/gi,
  )) {
    enabled.add(`${match[1].toLowerCase()}.${match[2].toLowerCase()}`);
  }
  for (const match of rlsSql.matchAll(
    /CREATE POLICY\s+\S+\s+ON\s+([a-z_][a-z0-9_]*)\.([a-z_][a-z0-9_]*)/gi,
  )) {
    const table = `${match[1].toLowerCase()}.${match[2].toLowerCase()}`;
    policies.set(table, (policies.get(table) ?? 0) + 1);
  }

  return [...tables]
    .sort()
    .map((table) => ({
      table,
      rls: enabled.has(table),
      policies: policies.get(table) ?? 0,
    }));
}

/** Tablas sin RLS habilitado o sin ninguna policy. */
export function missingRlsTables(root) {
  return readRlsStatus(root)
    .filter((table) => !table.rls || table.policies === 0)
    .map((table) => table.table);
}

/**
 * Compara el estado RLS real con el baseline de deuda explícita.
 *
 * @param {string} root
 * @param {string[]} baseline
 * @returns {string[]}
 */
export function checkRlsCoverage(root, baseline) {
  const baselineSet = new Set(baseline);
  const status = readRlsStatus(root);
  const missing = new Set(
    status
      .filter((table) => !table.rls || table.policies === 0)
      .map((table) => table.table),
  );
  const existing = new Set(status.map((table) => table.table));
  const problems = [];

  for (const table of status) {
    if (!missing.has(table.table) && baselineSet.has(table.table)) {
      problems.push(
        `${table.table}: ya tiene RLS y policy; elimina la excepción del baseline`,
      );
    }
  }

  for (const table of missing) {
    if (!baselineSet.has(table)) {
      problems.push(`${table}: sin RLS o sin policy y no está en el baseline (R-SB-2)`);
    }
  }

  for (const table of baselineSet) {
    if (!existing.has(table)) {
      problems.push(`${table}: en el baseline pero ya no existe como tabla`);
    }
  }

  return problems;
}
