/**
 * config/project.mjs — Core Engine
 * ──────────────────────────────────────────────────────────────────────────────
 * Datos específicos de ESTE proyecto para el motor de reglas (`scripts/engine/`).
 * Es la capa de configuración: solo datos, sin código ni dependencias del motor.
 * Al portar el motor a otro repositorio del stack se edita este archivo (y
 * `manifest.mjs`); `scripts/engine/` no se toca.
 *
 * Core Engine es un **monorepo npm workspaces**: varios servicios NestJS en
 * `packages/*` comparten `packages/shared`. Por eso la configuración es
 * multi-paquete (lista de servicios) y no un único `APP_DIR`.
 * ──────────────────────────────────────────────────────────────────────────────
 */

/** Rama principal del repositorio. */
export const MAIN_BRANCH = 'main';

/** Rama desplegada de aceptación previa a producción. */
export const STAGE_BRANCH = 'staging';

/** Owner de GitHub Container Registry usado por los composes de stage/prod. */
export const GHCR_OWNER = 'xozan845-ctrl';

/**
 * Rama de integración (R-GE-1): el trabajo diario entra por PR a `develop`, y
 * sube a `staging` y `main` por promoción.
 */
export const INTEGRATION_BRANCH = 'develop';

/** Directorio de los paquetes del workspace. */
export const PACKAGES_DIR = 'packages';

/** Árbol normativo de reglas (lo lee el motor para el reporte de cobertura). */
export const RULES_DIR = 'docs/rules';

/** Directorio de los workflows de GitHub Actions. */
export const WORKFLOWS_DIR = '.github/workflows';

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
  'scheduling-service',
  'commissions-service',
  'finance-service',
  'field-service',
  'market-intelligence-service',
  'storage-service',
  'notifications-service',
];

/**
 * Excepciones temporales de R-QA-1. Deben estar justificadas en la regla y la
 * suite `microservices-ci-contract` impide que oculten una suite ya existente.
 */
export const SERVICE_TEST_EXCEPTIONS = [];

/**
 * Servicios con implementación Rust (crate + cargo). Quedan fuera del runner
 * Vitest y del contrato de scripts npm; su gate vive en el job `rust` de CI
 * (ADR-0004, ADR-0006, ADR-0008, ADR-0010 y docs/rules/test/06-estandares-cobertura.md).
 */
export const RUST_SERVICES = [
  'scheduling-service',
  'notifications-service',
  'commissions-service',
  'field-service',
  'logistics-service',
  'market-intelligence-service',
  'stores-service',
];

/** Servicios Node/NestJS con suite Vitest (todos menos los migrados a Rust). */
export const SERVICES_NODE = SERVICES.filter(
  (service) => !RUST_SERVICES.includes(service),
);

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
  'auth',
  'personal',
  'catalog',
  'catalog-service',
  'stores',
  'stores-service',
  'orders',
  'orders-service',
  'logistics',
  'logistics-service',
  'scheduling',
  'scheduling-service',
  'commissions',
  'commissions-service',
  'finance',
  'finance-service',
  'field',
  'field-service',
  'market-intelligence',
  'market-intelligence-service',
  'storage',
  'storage-service',
  'notifications',
  'notifications-service',
  // Infraestructura y transversal
  'infra',
  'api',
  'compose',
  'db',
  'data',
  'observability',
  'security',
  'deps',
  // Gobernanza y entrega
  'rules',
  'docs',
  'plan',
  'ci',
  'cd',
  'release',
  'git',
  'test',
  'integration',
];
