const { execSync } = require('child_process');
const fs = require('fs');

const ENV = fs.readFileSync('C:/Users/User/Desktop/core-engine/.env', 'utf8');
const m = ENV.match(/^GRAFANA_ADMIN_PASSWORD=(.+)$/m);
const PW = (m ? m[1] : 'admin').trim();
const GF_URL = 'http://localhost:3000';
const PROM_URL = 'http://localhost:9090';

function promQuery(q) {
  try {
    const url = `${PROM_URL}/api/v1/query?query=${encodeURIComponent(q)}`;
    const out = execSync(`docker exec core-engine-prometheus wget -qO- "${url}"`, {
      encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], timeout: 20000,
    });
    return JSON.parse(out.trim());
  } catch (e) {
    return { status: 'error', error: String(e).slice(0, 300) };
  }
}

function grafanaGet(path) {
  const out = execSync(`curl.exe -s -u "admin:${PW}" "${GF_URL}${path}"`, {
    encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], timeout: 20000,
  });
  return JSON.parse(out.trim());
}

function flatPanels(panels, out = []) {
  for (const p of panels || []) {
    out.push(p);
    if (p.panels && p.panels.length) flatPanels(p.panels, out);
  }
  return out;
}

const PROM_KEYWORDS = new Set([
  'sum','rate','count','max','min','avg','topk','bottomk','by','without','on','ignoring',
  'group_left','group_right','unless','and','or','bool','offset','clamp_min','clamp_max',
  'histogram_quantile','abs','ceil','floor','round','exp','ln','log2','log10','sqrt',
  'deriv','predict_linear','increase','delta','idelta','label_replace','label_join',
  'sort','sort_desc','sort_by_label','sort_by_label_desc','cos','sin','tan','deg','rad',
  'year','month','day_of_week','day_of_month','days_in_month','hour','minute','time',
  'timestamp','vector','scalar','sgn','last_over_time','first_over_time','max_over_time',
  'min_over_time','avg_over_time','sum_over_time','stddev_over_time','stdvar_over_time',
  'count_over_time','quantile_over_time','absent','present_over_time','changes','resets',
  'stddev','stdvar','quantile','group','count_values','acos','asin','atan',
]);

// Extrae SOLO nombres de metricas: elimina strings, blocks de labels {} y rangos []
function extractMetricNames(expr) {
  let s = expr.replace(/"(?:[^"\\]|\\.)*"/g, '""').replace(/'(?:[^'\\]|\\.)*'/g, "''");
  s = s.replace(/\b(by|without|on|ignoring|group_left|group_right)\s*\([^)]*\)/g, ''); // quita labels de agrupacion
  s = s.replace(/\{[^{}]*\}/g, '');       // quita matchers de labels
  s = s.replace(/\[[^\]]*\]/g, '');       // quita rangos de tiempo
  const tokens = s.match(/[a-zA-Z_:][a-zA-Z0-9_:]*/g) || [];
  return [...new Set(tokens.filter(t => !PROM_KEYWORDS.has(t)))];
}

// Metrica condicionales legitimas definidas en queries.yaml del postgres-exporter
// (solo crean series cuando hay filas: vacuum activo, bloqueos, replicas, temp spills)
const condicionalesPG = new Set(
  fs.readFileSync('C:/Users/User/Desktop/core-engine/infra/prometheus/queries.yaml', 'utf8')
    .match(/^[a-z_]+:$/gm).map(k => k.replace(':', '').trim())
    .map(ns => ns.replace(/^pg_/, ''))
    // reconstruimos: pg_<namespace>_<columna> ya son runtime; aqui solo marcamos el prefijo del namespace
    .map(ns => ns)
);
// Prefijos de namespaces condicionales definidos en queries.yaml
const condPrefixes = fs.readFileSync('C:/Users/User/Desktop/core-engine/infra/prometheus/queries.yaml', 'utf8')
  .match(/^[a-z_]+:$/gm).map(k => k.replace(':', '').trim());

function getMetricCatalog() {
  const url = `${PROM_URL}/api/v1/label/__name__/values`;
  const out = execSync(`docker exec core-engine-prometheus wget -qO- "${url}"`, {
    encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], timeout: 30000,
  });
  const r = JSON.parse(out.trim());
  return new Set(r.data || []);
}

const catalog = getMetricCatalog();
console.log(`Catalogo de metricas Prometheus: ${catalog.size}\n`);

const dashboards = grafanaGet('/api/search').filter(d => d.type === 'dash-db');
let dashOk = 0, dashIssues = 0;

for (const db of dashboards) {
  const full = grafanaGet(`/api/dashboards/uid/${db.uid}`);
  const dash = full.dashboard;
  const panels = flatPanels(dash.panels);
  let pOk = 0, pIssues = 0, pSkip = 0, pNoData = 0;
  const issues = [];

  for (const p of panels) {
    if (p.type === 'row') continue;
    const targets = p.targets || [];
    if (targets.length === 0) { pSkip++; continue; }
    const title = p.title || '(sin titulo)';

    for (const t of targets) {
      if (!t.expr) continue;
      const r = promQuery(t.expr);
      if (r.status !== 'success') {
        pIssues++;
        issues.push(`  [ERROR SYNTAX] ${title}\n      expr: ${t.expr}\n      -> ${(r.error || r.status).replace(/\n/g, ' ')}`);
        continue;
      }
      const hasData = r.data && r.data.result && r.data.result.length > 0;
      if (hasData) { pOk++; continue; }

      pNoData++;
      const names = extractMetricNames(t.expr);
      const missing = names.filter(n => !catalog.has(n));
      const condicional = names.every(n =>
        catalog.has(n) || condPrefixes.some(pfx => n.startsWith(`${pfx}_`) || n === pfx)
      );
      if (missing.length > 0 && !condicional) {
        pIssues++;
        issues.push(`  [METRICA INEXISTENTE] ${title}\n      expr: ${t.expr}\n      -> no existe: ${missing.join(', ')}`);
      }
      // si es condicional legitima: OK (la metrica aparece bajo actividad)
    }
  }

  const statusLine = `  ${dash.title}: ${pOk} con datos, ${pNoData} sin datos, ${pIssues} problemas, ${pSkip} sin targets`;
  if (pIssues > 0) {
    dashIssues++;
    console.log(`[ISSUES] ${statusLine}`);
    issues.forEach(i => console.log(i));
  } else {
    dashOk++;
    console.log(`[OK]    ${statusLine}`);
  }
}

console.log('');
console.log(`Resumen: ${dashOk} dashboards OK, ${dashIssues} con problemas`);