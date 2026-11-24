#!/usr/bin/env node
/**
 * architecture-baseline.mjs — mantiene la deuda arquitectónica congelada.
 *
 * Uso:
 *   node scripts/tools/architecture-baseline.mjs            # muestra el delta
 *   node scripts/tools/architecture-baseline.mjs --update   # guarda el estado
 *
 * `baseline:guard` sigue siendo la barrera de CI: este comando no autoriza
 * deuda nueva, sólo evita editar el JSON a mano.
 */
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import {
  ARCHITECTURE_BASELINE_FILE,
  collectConformance,
  findingKey,
  loadArchitectureBaseline,
} from '../config/service-conformance.mjs';

const update = process.argv.includes('--update');
const { findings } = collectConformance();
const current = [...new Set(findings.filter((finding) => finding.baselineable).map(findingKey))].sort();
const baseline = loadArchitectureBaseline();
const previous = [...(baseline.debt ?? [])].sort();

const added = current.filter((key) => !previous.includes(key));
const removed = previous.filter((key) => !current.includes(key));

console.log(`Deuda arquitectónica: ${previous.length} → ${current.length}`);
for (const key of added) console.log(`  + ${key}`);
for (const key of removed) console.log(`  - ${key}`);

if (!update) {
  if (added.length > 0) {
    console.error('❌ Hay deuda nueva. Corrige el hallazgo o justifícala en el ADR; no se congela sin revisión (R-AF-2).');
    process.exit(1);
  }
  console.log('✅ No hay deuda nueva. Usa --update para retirar entradas ya corregidas.');
  process.exit(0);
}

const document = {
  version: 1,
  policy:
    'Deuda arquitectónica congelada (R-AF-2). Sólo puede reducirse: una entrada nueva la rechaza baseline:guard y una entrada obsoleta la rechaza services-contract.',
  debt: current,
};
writeFileSync(join(repoRoot, ARCHITECTURE_BASELINE_FILE), `${JSON.stringify(document, null, 2)}\n`);
console.log(`✅ ${ARCHITECTURE_BASELINE_FILE} actualizado.`);
