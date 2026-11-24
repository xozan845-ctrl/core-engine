#!/usr/bin/env node
/**
 * ci-matrix.mjs — emite la matriz Rust de CI desde los manifiestos.
 *
 * Sustituye la lista hardcodeada de `.github/workflows/ci.yml` (servicio,
 * directorio y umbrales de cobertura). Así un servicio nuevo solo se declara
 * en `packages/<servicio>/service.json` y la matriz se actualiza sola.
 *
 * Uso:
 *   node scripts/tools/ci-matrix.mjs            # JSON de una línea para $GITHUB_OUTPUT
 *   node scripts/tools/ci-matrix.mjs --pretty   # legible para depurar
 */
import { coverageMatrix } from '../config/services.mjs';

const pretty = process.argv.includes('--pretty');
const matrix = coverageMatrix();

if (pretty) {
  console.log(JSON.stringify(matrix, null, 2));
} else {
  // stdout debe ser una sola línea: GitHub Actions lo escribe en $GITHUB_OUTPUT.
  // Se emite el array (no { include }) porque `matrix.include` espera una lista.
  console.log(JSON.stringify(matrix));
}
