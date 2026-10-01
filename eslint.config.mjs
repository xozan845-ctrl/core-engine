// Configuración ESLint (flat config) — única para todos los workspaces.
// Alcance (G-7): TypeScript de los workspaces, `packages/*/src/**/*.ts`.
// El tooling JS (`qa-harness/`, `scripts/`, `validate-dashboards.cjs`) queda
// fuera de alcance por ahora — deuda anotada en docs/rules/test/README.md.
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
      'scripts/**',
      'validate-dashboards.cjs',
    ],
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
    // Globals de Jest solo en specs (describe/it/expect...).
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
        jest: 'readonly',
      },
    },
  },
);
