#!/usr/bin/env node
/**
 * k6-compare.mjs — línea base y gate de no-regresión de k6.
 *
 * Uso:
 *   node scripts/tools/k6-compare.mjs --summary tests/k6/k6-summary.json --env stage
 *   node scripts/tools/k6-compare.mjs --update --summary ... --env stage --rate 20 --duration 2m
 *
 * Compara p95, p99, error rate y RPS contra `scripts/config/k6-baseline.json`.
 * `--allow-missing` convierte la ausencia de baseline en un aviso (para el
 * primer despliegue, antes de calibrar).
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { repoRoot } from '../engine/lib/paths.mjs';

function parseArgs(argv) {
  const opciones = {
    summary: join(repoRoot, 'tests/k6/k6-summary.json'),
    env: 'local',
    scenario: 'load',
    baseline: join(repoRoot, 'scripts/config/k6-baseline.json'),
    update: false,
    allowMissing: false,
    rate: undefined,
    duration: undefined,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--summary') opciones.summary = argv[++i];
    else if (arg === '--env') opciones.env = argv[++i];
    else if (arg === '--scenario') opciones.scenario = argv[++i];
    else if (arg === '--baseline') opciones.baseline = argv[++i];
    else if (arg === '--rate') opciones.rate = argv[++i];
    else if (arg === '--duration') opciones.duration = argv[++i];
    else if (arg === '--update') opciones.update = true;
    else if (arg === '--allow-missing') opciones.allowMissing = true;
    else if (arg === '--help' || arg === '-h') {
      console.log(
        'Uso: node scripts/tools/k6-compare.mjs [--summary <json>] [--env local|stage|prod] [--scenario load] [--update] [--allow-missing] [--rate <rps>] [--duration <t>]',
      );
      process.exit(0);
    } else {
      throw new Error(`Argumento no reconocido: ${arg}`);
    }
  }
  return opciones;
}

const opciones = parseArgs(process.argv.slice(2));

if (!existsSync(opciones.summary)) {
  console.error(`❌ No existe el summary de k6: ${opciones.summary}`);
  process.exit(1);
}

const summary = JSON.parse(readFileSync(opciones.summary, 'utf8'));
const valores = (nombre) => summary.metrics?.[nombre]?.values ?? {};
const p95 = valores('http_req_duration')['p(95)'];
const p99 = valores('http_req_duration')['p(99)'];
const errorRate = valores('http_req_failed').rate;
const rps = valores('http_reqs').rate;

if ([p95, p99, errorRate, rps].some((valor) => typeof valor !== 'number' || Number.isNaN(valor))) {
  console.error('❌ El summary no contiene las métricas esperadas (http_req_duration/http_req_failed/http_reqs).');
  process.exit(1);
}

const baseline = existsSync(opciones.baseline)
  ? JSON.parse(readFileSync(opciones.baseline, 'utf8'))
  : { local: {}, stage: {}, prod: {} };

const redondear = (valor, decimales = 0) => Number(valor.toFixed(decimales));
const actual = {
  p95: redondear(p95),
  p99: redondear(p99),
  errorRate: redondear(errorRate, 4),
  rps: redondear(rps, 2),
};

if (opciones.update) {
  baseline[opciones.env] ??= {};
  baseline[opciones.env][opciones.scenario] = {
    ...actual,
    minRps: redondear(actual.rps * 0.8, 2),
    ...(opciones.rate ? { rate: Number(opciones.rate) } : {}),
    ...(opciones.duration ? { duration: opciones.duration } : {}),
    updatedAt: new Date().toISOString(),
  };
  writeFileSync(opciones.baseline, `${JSON.stringify(baseline, null, 2)}\n`);
  console.log(
    `✅ Baseline ${opciones.env}/${opciones.scenario} actualizada: p95=${actual.p95} ms, p99=${actual.p99} ms, error=${(actual.errorRate * 100).toFixed(2)} %, rps=${actual.rps}.`,
  );
  process.exit(0);
}

const referencia = baseline[opciones.env]?.[opciones.scenario];
if (!referencia) {
  const mensaje = `No hay baseline para ${opciones.env}/${opciones.scenario} en ${opciones.baseline}.`;
  if (opciones.allowMissing) {
    console.warn(`⚠️  ${mensaje} Se omite el gate (--allow-missing).`);
    process.exit(0);
  }
  console.error(`❌ ${mensaje} Calibra con --update.`);
  process.exit(2);
}

const problemas = [];
const p95Limite = referencia.p95 * 1.25;
const p99Limite = referencia.p99 * 1.3;
const errorLimite = Math.max(referencia.errorRate + 0.01, 0.01);
const rpsMinimo = referencia.minRps ?? referencia.rps * 0.8;

if (actual.p95 > p95Limite) {
  problemas.push(`p95 ${actual.p95} ms > ${redondear(p95Limite)} ms (baseline ${referencia.p95}, +25 %)`);
}
if (actual.p99 > p99Limite) {
  problemas.push(`p99 ${actual.p99} ms > ${redondear(p99Limite)} ms (baseline ${referencia.p99}, +30 %)`);
}
if (actual.errorRate > errorLimite) {
  problemas.push(
    `error rate ${(actual.errorRate * 100).toFixed(2)} % > ${(errorLimite * 100).toFixed(2)} %`,
  );
}
if (actual.rps < rpsMinimo) {
  problemas.push(`rps ${actual.rps} < ${redondear(rpsMinimo, 2)} (mínimo de la baseline)`);
}

console.log(`\nComparación ${opciones.env}/${opciones.scenario}`);
console.log('métrica    actual      baseline    límite');
console.log(
  `p95        ${String(actual.p95).padEnd(11)} ${String(referencia.p95).padEnd(11)} ${redondear(p95Limite)}`,
);
console.log(
  `p99        ${String(actual.p99).padEnd(11)} ${String(referencia.p99).padEnd(11)} ${redondear(p99Limite)}`,
);
console.log(
  `error      ${(actual.errorRate * 100).toFixed(2).padEnd(11)} ${(referencia.errorRate * 100).toFixed(2).padEnd(11)} ${(errorLimite * 100).toFixed(2)}`,
);
console.log(
  `rps        ${String(actual.rps).padEnd(11)} ${String(referencia.rps).padEnd(11)} ${redondear(rpsMinimo, 2)}`,
);

if (problemas.length > 0) {
  console.error(`\n❌ Regresión de rendimiento:\n- ${problemas.join('\n- ')}`);
  process.exit(1);
}

console.log('\n✅ Sin regresión frente a la línea base.');
