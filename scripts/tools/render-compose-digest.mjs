#!/usr/bin/env node
/**
 * render-compose-digest.mjs — fija las imágenes de un entorno por digest.
 *
 * Uso:
 *   node scripts/tools/render-compose-digest.mjs --source stage --output docker-compose.digest.yml
 *   node scripts/tools/render-compose-digest.mjs --source v1.12.0 --environment prod
 *
 * Requiere `docker buildx` autenticado contra GHCR. Genera un override de
 * Compose con `image: <repo>@sha256:...` para que un redeploy no cambie el
 * artefacto aunque la etiqueta móvil se mueva.
 */
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { GHCR_OWNER, SERVICES } from '../config/project.mjs';
import { repoRoot } from '../engine/lib/paths.mjs';

function parseArgs(argv) {
  const options = {
    source: undefined,
    environment: 'prod',
    output: undefined,
    help: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--source') options.source = argv[++i];
    else if (arg.startsWith('--source=')) options.source = arg.slice('--source='.length);
    else if (arg === '--environment') options.environment = argv[++i];
    else if (arg.startsWith('--environment=')) options.environment = arg.slice('--environment='.length);
    else if (arg === '--output') options.output = argv[++i];
    else if (arg.startsWith('--output=')) options.output = arg.slice('--output='.length);
    else if (arg === '--help' || arg === '-h') options.help = true;
    else throw new Error(`Argumento no reconocido: ${arg}`);
  }
  options.source ??= options.environment === 'stage' ? 'stage' : 'prod';
  options.output ??= join(repoRoot, `docker-compose.digest.${options.environment}.yml`);
  return options;
}

function digest(image) {
  const salida = execFileSync(
    'docker',
    ['buildx', 'imagetools', 'inspect', image, '--format', '{{.Manifest.Digest}}'],
    { encoding: 'utf8' },
  ).trim();
  if (!/^sha256:[0-9a-f]{64}$/.test(salida)) {
    throw new Error(`digest inválido para ${image}: ${salida}`);
  }
  return salida;
}

const options = parseArgs(process.argv.slice(2));
if (options.help) {
  console.log('Uso: render-compose-digest.mjs [--source <tag>] [--environment prod|stage] [--output <archivo>]');
  process.exit(0);
}

const lineas = ['# Generado por render-compose-digest.mjs; no editar a mano.', 'services:'];
for (const servicio of SERVICES) {
  const imagen = `ghcr.io/${GHCR_OWNER}/core-engine-${servicio}`;
  const fijado = `${imagen}@${digest(`${imagen}:${options.source}`)}`;
  lineas.push(`  ${servicio}:`);
  lineas.push(`    image: ${fijado}`);
}
writeFileSync(options.output, `${lineas.join('\n')}\n`);
console.log(`✅ ${SERVICES.length} imágenes fijadas por digest en ${options.output}`);
