/**
 * Salida del motor: plan de suites y reporte de cobertura de reglas. La
 * cobertura hace visible la deuda: qué IDs tienen check ejecutable y qué IDs
 * citan las suites sin estar definidos en `docs/rules` (hallazgo).
 */
import { relative } from 'node:path';
import { repoRoot } from './lib/paths.mjs';
import { collectRuleIds } from './lib/rules-docs.mjs';

export function printSuitePlan(suites) {
  console.log(`Ejecutando validación modular de reglas (${suites.length} suites)...\n`);
  for (const suite of suites) {
    const rules = suite.rules.length > 0 ? suite.rules.join(', ') : '—';
    console.log(
      `  • ${suite.id}  [${suite.mode}]  ${rules}  — ${relative(repoRoot, suite.file)}`,
    );
  }
  console.log('');
}

export function printCoverage({ suites, rulesRoot }) {
  const defined = collectRuleIds(rulesRoot);
  const cited = new Set(suites.flatMap((suite) => suite.rules));
  const covered = [...cited].filter((id) => defined.has(id)).sort();
  const unknown = [...cited].filter((id) => !defined.has(id)).sort();

  console.log('');
  console.log(
    `Cobertura ejecutable: ${covered.length}/${defined.size} IDs de regla tienen check.`,
  );
  if (unknown.length > 0) {
    console.log(
      `  ⚠ Hallazgo — IDs citados por suites sin definición en docs/rules: ${unknown.join(', ')}`,
    );
  }
}
