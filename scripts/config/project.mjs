/**
 * config/project.mjs — Core Engine
 * ──────────────────────────────────────────────────────────────────────────────
 * Datos específicos de ESTE proyecto para el motor de reglas (`scripts/engine/`).
 * Es la capa de configuración: solo datos, sin código ni dependencias del motor.
 * Al portar el motor a otro repositorio del stack se edita este archivo (y
 * `manifest.mjs`); `scripts/engine/` no se toca.
 *
 * Core Engine es un **monorepo de crates Rust independientes**. El inventario
 * de servicios ya **no se hardcodea aquí**: se deriva de
 * `packages/<servicio>/service.json` (R-MS-9) a través de `services.mjs`.
 * `SERVICES` y `RUST_SERVICES` se reexportan para conservar la API del motor.
 * ──────────────────────────────────────────────────────────────────────────────
 */
import {
  PACKAGES_DIR,
  RUST_SERVICES,
  SERVICES,
  SERVICE_MANIFESTS,
  SERVICE_PORTS,
  SERVICE_PROFILES,
} from './services.mjs';

export {
  PACKAGES_DIR,
  RUST_SERVICES,
  SERVICES,
  SERVICE_MANIFESTS,
  SERVICE_PORTS,
  SERVICE_PROFILES,
};

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

/** Árbol normativo de reglas (lo lee el motor para el reporte de cobertura). */
export const RULES_DIR = 'docs/rules';

/** Directorio de los workflows de GitHub Actions. */
export const WORKFLOWS_DIR = '.github/workflows';

/**
 * Excepciones temporales de R-QA-1. Deben estar justificadas en la regla y la
 * suite `microservices-ci-contract` impide que oculten una suite ya existente.
 */
export const SERVICE_TEST_EXCEPTIONS = [];

/** Servicios Node/NestJS con suite Vitest (hoy vacío: todos migraron a Rust). */
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
  'gateway',
  'shared',
  'api-gateway',
  'identity',
  'identity-service',
  'auth',
  'personal',
  'catalog',
  'item-service',
  'inventory',
  'inventory-service',
  'pricing',
  'pricing-service',
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
  'analytics',
  'analytics-service',
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
  'architecture',
  'audit',
  'docs',
  'plan',
  'ci',
  'cd',
  'release',
  'git',
  'test',
  'integration',
  'retail',
];
