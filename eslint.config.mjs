// Configuración ESLint (flat config) — única para todos los workspaces.
// Alcance (G-7): TypeScript de los workspaces, `packages/*/src/**/*.ts`.
// El tooling JS (`qa-harness/`) queda fuera de alcance; el motor de gobernanza
// (`scripts/**/*.mjs`) SÍ se lintea.
// Uso: `npm run lint` (raíz). Los errores bloquean CI; las advertencias no.
import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: [
      '**/dist/**',
      '**/coverage/**',
      '**/node_modules/**',
      '**/*.tsbuildinfo',
      '**/*.d.ts',
      'qa-harness/**',
    ],
  },
  {
    // Motor de gobernanza y herramientas: JS plano (ESM) sobre Node.
    files: ['scripts/**/*.mjs'],
    extends: [js.configs.recommended],
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
  {
    files: ['packages/*/src/**/*.ts'],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    rules: {
      // Los `any` explícitos se toleran como advertencia: se auditan en R-COV-3.
      '@typescript-eslint/no-explicit-any': 'warn',
      // Underscore inicial = parámetro/variable intencionalmente ignorado.
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' },
      ],
    },
  },
  {
    // Globals de Vitest solo en specs (describe/it/expect/vi...).
    files: ['packages/*/src/**/*.spec.ts'],
    languageOptions: {
      globals: {
        describe: 'readonly',
        it: 'readonly',
        test: 'readonly',
        expect: 'readonly',
        beforeEach: 'readonly',
        afterEach: 'readonly',
        beforeAll: 'readonly',
        afterAll: 'readonly',
        vi: 'readonly',
      },
    },
  },
);
