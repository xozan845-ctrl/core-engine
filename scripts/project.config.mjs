/**
 * project.config.mjs — Core Engine
 * ──────────────────────────────────────────────────────────────────────────────
 * Configuración específica de ESTE proyecto para el motor de reglas
 * (`scripts/check-rules.mjs`). Es el único archivo a editar al portar el motor
 * a otro repositorio del stack.
 *
 * Core Engine es un **monorepo npm workspaces**: varios servicios NestJS en
 * `packages/*` comparten `packages/shared`. Por eso la configuración es
 * multi-paquete (lista de servicios) y no un único `APP_DIR`.
 * ──────────────────────────────────────────────────────────────────────────────
 */

/** Rama principal del repositorio. */
export const MAIN_BRANCH = 'main';

/** Owner de GitHub Container Registry usado por los composes de stage/prod. */
export const GHCR_OWNER = 'xozan845-ctrl';

/**
 * Rama de integración (R-GE-1): el trabajo diario entra por PR a `develop`, y
 * sube a `staging` y `main` por promoción.
 */
export const INTEGRATION_BRANCH = 'develop';

/** Directorio de los paquetes del workspace. */
export const PACKAGES_DIR = 'packages';

/**
 * Servicios desplegables del monorepo (uno por carpeta `packages/<servicio>`
 * con `Dockerfile` propio). `shared` es librería, no servicio.
 */
export const SERVICES = [
  'api-gateway',
  'identity-service',
  'catalog-service',
  'stores-service',
  'orders-service',
  'logistics-service',
  'commissions-service',
  'finance-service',
  'field-service',
  'market-intelligence-service',
];

/**
 * Tipos de Conventional Commits admitidos (R-GC-1).
 */
export const ALLOWED_COMMIT_TYPES = [
  'feat',
  'fix',
  'docs',
  'test',
  'refactor',
  'perf',
  'style',
  'ci',
  'build',
  'chore',
  'revert',
];

/**
 * Ámbitos reales del repo (R-GC-1): servicios, infraestructura y áreas de
 * gobernanza. El ámbito es obligatorio.
 */
export const ALLOWED_COMMIT_SCOPES = [
  // Servicios (nombre corto y nombre completo del paquete)
  'shared',
  'gateway',
  'api-gateway',
  'identity',
  'identity-service',
  'catalog',
  'catalog-service',
  'stores',
  'stores-service',
  'orders',
  'orders-service',
  'logistics',
  'logistics-service',
  'commissions',
  'commissions-service',
  'finance',
  'finance-service',
  'field',
  'field-service',
  'market-intelligence',
  'market-intelligence-service',
  // Infraestructura y transversal
  'infra',
  'compose',
  'db',
  'data',
  'observability',
  'security',
  'deps',
  // Gobernanza y entrega
  'rules',
  'docs',
  'ci',
  'cd',
  'release',
  'git',
  'test',
];

/**
 * Manifiesto de suites de reglas (R-CI-9): `check-rules.mjs` descubre las
 * suites por `readdir`, así que borrar una en silencio reduciría la cobertura.
 * Esta lista hace que `rules-check` falle si falta cualquiera.
 */
export const REQUIRED_RULE_SUITES = [
  'commits.test.mjs',
  'delivery.test.mjs',
  'meta.test.mjs',
  'services.test.mjs',
];
