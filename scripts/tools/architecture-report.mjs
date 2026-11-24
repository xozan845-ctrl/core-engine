#!/usr/bin/env node
/**
 * architecture-report.mjs — genera la documentación derivada del catálogo.
 *
 * Salidas (todas con cabecera GENERADO, nunca se editan a mano — R-AF-4):
 *   - docs/architecture/catalogo-servicios.md
 *   - docs/architecture/matriz-eventos.md
 *   - docs/architecture/deuda-arquitectura.md
 *
 * Uso:
 *   node scripts/tools/architecture-report.mjs            # escribe
 *   node scripts/tools/architecture-report.mjs --check    # falla si hay diff
 */
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';
import {
  renderCatalogMarkdown,
  renderDebtMarkdown,
  renderEventMatrixMarkdown,
} from '../config/service-conformance.mjs';

const OUTPUT_DIR = 'docs/architecture';
const OUTPUTS = {
  'catalogo-servicios.md': renderCatalogMarkdown,
  'matriz-eventos.md': renderEventMatrixMarkdown,
  'deuda-arquitectura.md': renderDebtMarkdown,
};

const check = process.argv.includes('--check');

mkdirSync(join(repoRoot, OUTPUT_DIR), { recursive: true });

const drift = [];
for (const [file, render] of Object.entries(OUTPUTS)) {
  const path = join(repoRoot, OUTPUT_DIR, file);
  const expected = render();
  const actual = existsSync(path) ? readFileSync(path, 'utf8') : null;
  if (actual === expected) continue;
  if (check) {
    drift.push(`${OUTPUT_DIR}/${file}`);
  } else {
    writeFileSync(path, expected);
    console.log(`✅ ${OUTPUT_DIR}/${file}`);
  }
}

if (check) {
  if (drift.length > 0) {
    console.error(
      `❌ Documentación de arquitectura desactualizada (R-AF-4):\n- ${drift.join('\n- ')}\nEjecuta: npm run architecture:report`,
    );
    process.exit(1);
  }
  console.log('✅ Catálogo, matriz de eventos y deuda sincronizados.');
}
