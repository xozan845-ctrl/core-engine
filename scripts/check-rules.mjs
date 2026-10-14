import { run } from 'node:test';
import { spec } from 'node:test/reporters';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readdirSync } from 'node:fs';

const currentDir = dirname(fileURLToPath(import.meta.url));
const rulesDir = join(currentDir, 'rules');

const testFiles = readdirSync(rulesDir)
  .filter((file) => file.endsWith('.test.mjs'))
  .map((file) => join(rulesDir, file))
  .sort();

console.log(`Ejecutando validación modular de reglas (${testFiles.length} suites)...\n`);

const testStream = run({ files: testFiles });
testStream.compose(spec).pipe(process.stdout);

let hasFailure = false;
testStream.on('test:fail', () => {
  hasFailure = true;
});

testStream.on('end', () => {
  if (hasFailure) {
    process.exitCode = 1;
  }
});
