/**
 * Coherencia entre los compose y las plantillas de entorno (R-CD-7).
 *
 * Verifica que toda variable `${VAR}` usada por los compose exista en las tres
 * plantillas (`.env.example`, `.env.stage.example`, `.env.production.example`) y
 * que ningún secreto quede hardcodeado en un compose. `ENV_FILE` es la variable
 * que inyecta Dockploy para apuntar al `.env` del entorno y se excluye.
 */
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export const ENV_TEMPLATES = [
  '.env.example',
  '.env.stage.example',
  '.env.production.example',
];

export const COMPOSE_FILES = [
  'docker-compose.yml',
  'docker-compose.dev.yml',
  'docker-compose.stage.yml',
  'docker-compose.observability.yml',
];

const EXTERNAL_VARS = new Set(['ENV_FILE']);
const SECRET_KEY = /(PASSWORD|SECRET|TOKEN|API_KEY|PRIVATE_KEY)/i;

function envKeys(file) {
  const keys = new Set();
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    const match = /^\s*([A-Z][A-Z0-9_]*)\s*=/.exec(line);
    if (match && !line.trimStart().startsWith('#')) keys.add(match[1]);
  }
  return keys;
}

function composeVars(content) {
  return new Set([...content.matchAll(/\$\{([A-Z][A-Z0-9_]*)/g)].map((match) => match[1]));
}

function hardcodedSecrets(content) {
  const problems = [];
  const lines = content.split('\n');

  for (let i = 0; i < lines.length; i += 1) {
    const match = /^\s+([A-Z][A-Z0-9_]*)\s*:\s*(.+)$/.exec(lines[i]);
    if (!match || !SECRET_KEY.test(match[1])) continue;

    const value = match[2].trim().replace(/^["']|["']$/g, '');
    if (value && !value.includes('${') && !value.startsWith('$')) {
      problems.push(
        `${match[1]}: valor hardcodeado en la línea ${i + 1}; usa una variable de entorno (R-CD-7)`,
      );
    }
  }

  return problems;
}

/**
 * @param {string} root
 * @returns {string[]}
 */
export function checkEnv(root) {
  const problems = [];
  const templates = new Map();

  for (const name of ENV_TEMPLATES) {
    const file = join(root, name);
    if (!existsSync(file)) {
      problems.push(`${name}: falta la plantilla de entorno`);
      continue;
    }
    templates.set(name, envKeys(file));
  }

  const used = new Set();
  for (const name of COMPOSE_FILES) {
    const file = join(root, name);
    if (!existsSync(file)) continue;

    const content = readFileSync(file, 'utf8');
    for (const variable of composeVars(content)) used.add(variable);
    for (const problem of hardcodedSecrets(content)) {
      problems.push(`${name}: ${problem}`);
    }
  }

  for (const variable of [...used].sort()) {
    if (EXTERNAL_VARS.has(variable)) continue;
    for (const [name, keys] of templates) {
      if (!keys.has(variable)) {
        problems.push(`${name}: falta ${variable}, usada por los compose`);
      }
    }
  }

  return problems;
}
