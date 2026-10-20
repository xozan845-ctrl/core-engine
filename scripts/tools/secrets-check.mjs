#!/usr/bin/env node
/**
 * secrets-check.mjs — escaneo de secretos de alta confianza (R-DS-1).
 *
 * Uso:
 *   npm run security:secrets
 *
 * Solo lee archivos versionados (`git ls-files`) y busca patrones inequívocos
 * (claves AWS/GitHub/Slack/Google/Stripe, claves privadas). No intenta detectar
 * contraseñas de ejemplo o placeholders: para eso está la revisión de PR.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';

const PATTERNS = [
  ['AWS access key', /AKIA[0-9A-Z]{16}/],
  ['GitHub token', /gh[pousr]_[A-Za-z0-9]{36,}/],
  ['Slack token', /xox[baprs]-[A-Za-z0-9-]{10,}/],
  ['Google API key', /AIza[0-9A-Za-z_-]{35}/],
  ['Stripe live key', /sk_live_[0-9a-zA-Z]{24,}/],
  [
    'Private key',
    /-----BEGIN (?:RSA|EC|OPENSSH|DSA|PGP) PRIVATE KEY-----/,
  ],
];

const MAX_BYTES = 2 * 1024 * 1024;

function parseRoot(argv) {
  if (argv.includes('--help') || argv.includes('-h')) {
    console.log('Uso: node scripts/tools/secrets-check.mjs [--root <dir>]');
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

function main() {
  const root = parseRoot(process.argv.slice(2));
  const files = execFileSync('git', ['ls-files', '-z'], {
    cwd: root,
    encoding: 'utf8',
  })
    .split('\0')
    .filter(Boolean);

  const findings = [];

  for (const file of files) {
    let buffer;
    try {
      buffer = readFileSync(join(root, file));
    } catch {
      continue;
    }
    if (buffer.length > MAX_BYTES || buffer.includes(0)) continue;

    const lines = buffer.toString('utf8').split('\n');
    for (let i = 0; i < lines.length; i += 1) {
      for (const [label, pattern] of PATTERNS) {
        if (pattern.test(lines[i])) {
          findings.push(`${file}:${i + 1}: ${label}`);
        }
      }
    }
  }

  if (findings.length > 0) {
    console.error(`❌ Posibles secretos versionados (R-DS-1):\n- ${findings.join('\n- ')}`);
    process.exit(1);
  }

  console.log(`✅ Sin secretos de alta confianza en ${files.length} archivos versionados.`);
}

try {
  main();
} catch (error) {
  console.error(`❌ ${error.message}`);
  process.exit(1);
}
