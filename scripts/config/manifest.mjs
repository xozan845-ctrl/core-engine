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
  { id: 'delivery-compose', file: 'rules/delivery/compose.test.mjs' },
  { id: 'services-monorepo', file: 'rules/services/monorepo.test.mjs' },
];
