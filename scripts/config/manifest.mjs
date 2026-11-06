/**
 * config/manifest.mjs — Manifiesto de suites de reglas (R-CI-7).
 *
 * El runner descubre las suites por `readdir`, así que borrar una en silencio
 * reduciría la cobertura sin que nada avise. Este manifiesto hace que
 * `rules:check` falle si falta cualquiera de las suites obligatorias.
 *
 * `id`   — identificador que exporta la suite (`defineSuite({ id })`).
 * `file` — ruta relativa a `scripts/` (para la comprobación de existencia).
 */
export const requiredSuites = [
  { id: 'docs-structure', file: 'rules/docs/structure.test.mjs' },
  { id: 'git-commits', file: 'rules/git/commits.test.mjs' },
  { id: 'git-branch', file: 'rules/git/branch.test.mjs' },
  { id: 'database-migrations', file: 'rules/db/migrations.test.mjs' },
  { id: 'database-rls', file: 'rules/db/rls.test.mjs' },
  { id: 'delivery-compose', file: 'rules/delivery/compose.test.mjs' },
  { id: 'services-monorepo', file: 'rules/microservices/structure.test.mjs' },
  {
    id: 'microservices-ci-contract',
    file: 'rules/microservices/ci-contract.test.mjs',
  },
  {
    id: 'openapi-contract',
    file: 'rules/contract/openapi.test.mjs',
  },
  {
    id: 'observability-tracing',
    file: 'rules/observability/tracing.test.mjs',
  },
  {
    id: 'observability-telemetry',
    file: 'rules/observability/telemetry.test.mjs',
  },
  {
    id: 'security-excessive-data-exposure',
    file: 'rules/security/excessive-data-exposure.test.mjs',
  },
  { id: 'security-authz-matrix', file: 'rules/security/authz-matrix.test.mjs' },
  { id: 'security-headers-cors', file: 'rules/security/headers-cors.test.mjs' },
  { id: 'security-rate-limit', file: 'rules/security/rate-limit.test.mjs' },
  { id: 'security-sql-injection', file: 'rules/security/sql-injection.test.mjs' },
  { id: 'security-ssrf', file: 'rules/security/ssrf.test.mjs' },
  { id: 'security-uploads', file: 'rules/security/uploads.test.mjs' },
  { id: 'security-mass-assignment', file: 'rules/security/mass-assignment.test.mjs' },
  { id: 'security-sensitive-logs', file: 'rules/security/sensitive-logs.test.mjs' },
];
