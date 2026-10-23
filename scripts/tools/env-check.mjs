#!/usr/bin/env node
/**
 * env-check.mjs — coherencia compose ↔ plantillas de entorno (R-CD-7).
 *
 * Uso:
 *   npm run env:check
 */
import { repoRoot } from '../engine/lib/paths.mjs';
import { checkEnv } from '../engine/lib/env.mjs';

function parseRoot(argv) {
  if (argv.includes('--help') || argv.includes('-h')) {
    console.log('Uso: node scripts/tools/env-check.mjs [--root <dir>]');
    process.exit(0);
  }

  const index = argv.indexOf('--root');
  if (index !== -1) {
    if (!argv[index + 1]) throw new Error('--root requiere un directorio');
    return argv[index + 1];
  }

  const inline = argv.find((arg) => arg.startsWith('--root='));
  if (inline) return inline.slice('--root='.length);

  const unknown = argv.find((arg) => arg !== '--root' && !arg.startsWith('--root='));
  if (unknown) throw new Error(`Argumento no reconocido: ${unknown}`);

  return repoRoot;
}

try {
  const root = parseRoot(process.argv.slice(2));
  const problems = checkEnv(root);

  if (problems.length > 0) {
    console.error(`❌ Entornos inconsistentes (R-CD-7):\n- ${problems.join('\n- ')}`);
    process.exit(1);
  }

  console.log('✅ Compose y plantillas de entorno alineados; sin secretos hardcodeados.');
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
