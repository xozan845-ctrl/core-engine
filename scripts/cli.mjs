/**
 * cli.mjs — Único entrypoint del motor de gobernanza.
 *
 * Es el *composition root*: es la única capa que puede importar de todas las
 * demás (`engine/`, `config/`, `tools/`). Los `npm scripts` de la raíz y el
 * wrapper `check-rules.mjs` delegan aquí.
 *
 * Uso:
 *   node scripts/cli.mjs rules [--only <id|area>] [--mode pure|repo] [--reporter spec|tap|dot|junit]
 *   node scripts/cli.mjs smoke
 *   node scripts/cli.mjs swagger
 */
import { pathToFileURL } from 'node:url';
import { runRules } from './engine/runner.mjs';
import { rulesDir, resolveInRepo } from './engine/lib/paths.mjs';
import { requiredSuites } from './config/manifest.mjs';
import { RULES_DIR } from './config/project.mjs';

function parseFlags(args) {
  const flags = {};
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (!arg.startsWith('--')) continue;
    const key = arg.slice(2);
    const next = args[i + 1];
    if (next !== undefined && !next.startsWith('--')) {
      flags[key] = next;
      i += 1;
    } else {
      flags[key] = true;
    }
  }
  return flags;
}

async function rulesCommand(args) {
  const flags = parseFlags(args);
  return runRules({
    rulesDir,
    requiredSuites,
    docsRulesRoot: resolveInRepo(RULES_DIR),
    only: typeof flags.only === 'string' ? flags.only : undefined,
    mode: typeof flags.mode === 'string' ? flags.mode : undefined,
    reporter: typeof flags.reporter === 'string' ? flags.reporter : 'spec',
  });
}

const COMMANDS = {
  rules: rulesCommand,
  smoke: () => import('./tools/smoke.mjs'),
  swagger: () => import('./tools/swagger-aggregate.mjs').then((modulo) => modulo.main()),
};

/**
 * Ejecuta un comando del CLI.
 * @param {string[]} [argv] Argumentos (por defecto, los del proceso).
 * @returns {Promise<number>} Código de salida.
 */
export async function main(argv = process.argv.slice(2)) {
  const [command, ...rest] = argv;
  const handler = COMMANDS[command];

  if (!handler) {
    console.error(
      `Comando desconocido: ${command ?? '(ninguno)'}. Usa: ${Object.keys(COMMANDS).join(' | ')}`,
    );
    return 1;
  }

  const result = await handler(rest);
  return typeof result === 'number' ? result : 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.exitCode = await main();
}
