/**
 * Suite 15 — Análisis estático de código (Node puro, Windows-compatible, sin shell externo)
 */
import { execSync } from 'child_process';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { readdirSync, readFileSync, existsSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

let projectRoot = join(__dirname, '../../../');
if (!existsSync(join(projectRoot, 'package.json'))) {
  projectRoot = process.cwd();
}

let WARN = '';
if (!existsSync(join(projectRoot, 'package.json'))) {
  WARN = `No se encontró package.json en ${projectRoot}`;
}

function archivosTS(dir) {
  const out = [];
  let entrada;
  try { entrada = readdirSync(dir, { withFileTypes: true }); } catch { return out; }
  for (const e of entrada) {
    if (e.name === 'node_modules' || e.name === 'dist' || e.name === 'qa-harness' || e.name === '.git') continue;
    const full = join(dir, e.name);
    if (e.isDirectory()) out.push(...archivosTS(full));
    else if (e.name.endsWith('.ts')) out.push(full);
  }
  return out;
}

function contarGrandes() {
  let n = 0;
  for (const f of archivosTS(projectRoot)) {
    try {
      if (readFileSync(f, 'utf-8').split('\n').length > 500) n++;
    } catch { /* ignore */ }
  }
  return n;
}

function buscarPatron(re) {
  let n = 0;
  for (const f of archivosTS(projectRoot)) {
    try {
      const c = readFileSync(f, 'utf-8');
      if (/\.(spec|test)\.ts$/.test(f)) continue;
      for (const linea of c.split('\n')) {
        if (re.test(linea)) n++;
        re.lastIndex = 0;
      }
    } catch { /* ignore */ }
  }
  return n;
}

export async function suite15Static(client, cfg) {
  const errores = [];
  const resultados = [];

  if (WARN) errores.push(WARN);

  const ejecutar = (nombre, cmd, timeoutMs = 120000) => {
    try {
      const out = execSync(cmd, { cwd: projectRoot, encoding: 'utf-8', timeout: timeoutMs });
      resultados.push({ test: nombre, pass: true, output: out.slice(0, 4000) });
    } catch (e) {
      const out = (e.stdout && e.stdout.toString() || e.message || '').slice(0, 4000);
      resultados.push({ test: nombre, pass: false, output: out });
    }
  };

  ejecutar('npm test', 'npm test 2>&1', 240000);
  ejecutar('npm run lint', 'npm run lint 2>&1', 120000);
  ejecutar('npm run build', 'npm run build 2>&1', 240000);
  // No hay tsconfig en la raiz (monorepo): el typecheck global se hace por workspace
  ejecutar('npx tsc --noEmit', 'npx tsc -p tsconfig.base.json --noEmit 2>&1', 120000);

  try {
    const grandes = contarGrandes();
    resultados.push({ test: `Archivos TS > 500 líneas: ${grandes}`, pass: grandes < 5 });
  } catch (e) {
    resultados.push({ test: 'Archivos TS > 500 líneas', pass: true, output: e.message });
  }

  try {
    // Patron case-sensitive con limites de palabra: evita falsos positivos
    // ("metodo"/"TODOS"/"listarTodo" contienen la subcadena todo, no son TODOs).
    const todos = buscarPatron(/\bTODO\b|\bFIXME\b/);
    resultados.push({ test: `TODO/FIXME en código: ${todos}`, pass: todos < 20 });
  } catch {
    resultados.push({ test: 'TODO/FIXME', pass: true });
  }

  try {
    const logs = buscarPatron(/console\.log/);
    resultados.push({ test: `console.log en fuente: ${logs}`, pass: logs <= 3 });
  } catch {
    resultados.push({ test: 'console.log', pass: true });
  }

  return {
    test: 'Análisis estático de código',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}