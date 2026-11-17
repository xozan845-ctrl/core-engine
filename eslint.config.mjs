// Configuración ESLint (flat config) — única para todos los workspaces.
// Alcance (G-7): TypeScript de los workspaces, `packages/*/src/**/*.ts`.
// El tooling JS (`qa-harness/`) queda fuera de alcance; el motor de gobernanza
// (`scripts/**/*.mjs`) SÍ se lintea.
// Uso: `npm run lint` (raíz). Los errores bloquean CI; las advertencias no.
import js from '@eslint/js';

export default [
  {
    ignores: [
      '**/dist/**',
      '**/coverage/**',
      '**/.stryker-tmp/**',
      '**/reports/**',
      '**/node_modules/**',
      '**/*.tsbuildinfo',
      '**/*.d.ts',
      'qa-harness/**',
    ],
  },
  {
    ...js.configs.recommended,
    // Motor de gobernanza y herramientas: JS plano (ESM) sobre Node.
    files: ['scripts/**/*.mjs'],
    languageOptions: {
      ecmaVersion: 2023,
      sourceType: 'module',
      globals: {
        process: 'readonly',
        console: 'readonly',
        URL: 'readonly',
        fetch: 'readonly',
        setTimeout: 'readonly',
        clearTimeout: 'readonly',
        Buffer: 'readonly',
        AbortController: 'readonly',
        performance: 'readonly',
      },
    },
  },
];
