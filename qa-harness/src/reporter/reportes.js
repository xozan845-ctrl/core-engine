import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);

// ── Reporte ─────────────────────────────────────────────────────────────
export class Reporta {
  constructor(suite) {
    this.suites = {};
    this.totalTests = 0;
    this.totalPass = 0;
    this.totalFail = 0;
    this.totalSkipped = 0;
    this.timestamp = new Date().toISOString();
    this.errors = [];
    this.metadata = {
      version: '1.0.0',
      project: 'BodegaHub Core Engine',
      date: new Date().toISOString(),
    };
    this.htmlReport = '';
    this.markdownReport = '';
    this.jsonReport = '';
  }

  addSuiteResult(name, result) {
    this.suites[name] = result;
    if (result.pasos === undefined) result.pasos = 0;
    this.totalTests += result.tests || 0;
    this.totalPass += result.pasos || 0;
    this.totalFail += result.fallos || 0;
    this.totalSkipped += result.skipped || 0;
    if (result.fallos > 0) this.errors.push({ suite: name, fallos: result.fallos, timestamp: new Date().toISOString() });
  }

  generarJSON() {
    this.jsonReport = JSON.stringify({
      metadata: this.metadata,
      suites: this.suites,
      resumen: {
        totalTests: this.totalTests,
        pasos: this.totalPass,
        fallos: this.totalFail,
        saltos: this.totalSkipped,
        tasa: this.totalPass > 0 ? ((this.totalPass / this.totalTests) * 100).toFixed(1) + '%' : '100%',
        errores: this.errors,
      },
      timestamp: this.timestamp,
    }, null, 2);
    return this.jsonReport;
  }

  generarMarkdown() {
    let md = '# BodegaHub — Reporte de Testing QA\n';
    md += `_Fecha: ${this.timestamp}_\n`;
    md += `_Proyecto: BodegaHub Core Engine\n\n`;
    md += `## Resumen\n\n`;
    md += `- **Pruebas:** ${this.totalTests}\n`;
    md += `- **Éxito:** ${this.totalPass}\n`;
    md += `- **Fallos:** ${this.totalFail}\n`;
    md += `- **Saltos:** ${this.totalSkipped}\n`;
    md += `- **Tasa de éxito:** ${((this.totalPass / Math.max(1, this.totalTests)) * 100).toFixed(1)}%\n\n`;

    md += `## Detalles por suite\n\n`;
    for (const [n, r] of Object.entries(this.suites)) {
      md += `### ${n}\n\n`;
      md += `- **Resultado:** ${r.estado || (r.pasos > 0 ? 'OK' : 'FAIL')} (${r.pasos || 0} / ${r.fallos || 0})\n`;
      md += `- **Pruebas:** ${r.tests || 0}\n`;
      md += `- **Fallos:** ${r.fallos || 0}\n\n`;
    }

    this.markdownReport = md;
    return md;
  }

  generarHTML() {
    let html = `<!DOCTYPE html><html><head><meta charset="UTF-8"><title>BodegaHub QA Report</title>
<style>
body { font-family: sans-serif; max-width: 1200px; margin: 0 auto; padding: 20px; background: #1a1a2e; color: #e0e0e0; }
h1 { color: #e94560; } h2 { color: #0f3460; border-bottom: 2px solid #e94560; }
table { border-collapse: collapse; width: 100%; margin: 10px 0; background: #16213e; }
th, td { border: 1px solid #0f3460; padding: 8px 12px; text-align: left; }
th { background: #0f3460; color: #fff; }
.pass { color: #00ff00; }
.fail { color: #ff0000; }
.skip { color: #ffaa00; }
.resultado { padding: 10px; margin: 8px 0; border-radius: 5px; }
.resultado.ok { background: rgba(0,255,0,0.1); border: 1px solid #00ff00; }
.resultado.err { background: rgba(255,0,0,0.1); border: 1px solid #ff0000; }
.resultado.skip { background: rgba(255,170,0,0.1); border: 1px solid #ffaa00; }
.metrics { display: flex; gap: 20px; }
.metric { background: #16213e; border-radius: 8px; padding: 15px; text-align: center; flex: 1; }
.metric .val { font-size: 2em; font-weight: bold; }
.metric .lbl { font-size: 0.8em; color: #888; }
</style></head><body>
<h1>BodegaHub · Core Engine — Reporte de Testing QA</h1>
<p>Fecha: ${this.timestamp} | Proyecto: BodegaHub Core Engine</p>
<div class="metrics">
<div class="metric"><div class="val">${this.totalTests}</div><div class="lbl">Pruebas</div></div>
<div class="metric"><div class="val">${this.totalPass}</div><div class="lbl">Éxito</div></div>
<div class="metric"><div class="val">${this.totalFail}</div><div class="lbl">Fallos</div></div>
<div class="metric"><div class="val">${this.totalSkipped}</div><div class="lbl">Saltos</div></div>
</div>
<h2>Detalles por suite</h2>`;

    for (const [n, r] of Object.entries(this.suites)) {
      const cls = r.pasos > 0 ? 'ok' : (r.fallos > 0 ? 'err' : 'skip');
      html += `<div class="resultado ${cls}"><h3>${n}</h3>
<p>Resultado: ${r.estado || (r.pasos > 0 ? 'OK' : 'FAIL')} | Pruebas: ${r.tests || 0} | Éxito: ${r.pasos || 0} | Fallos: ${r.fallos || 0} | Saltos: ${r.skipped || 0}</p>
<p>Error(s): ${(r.fallos || 0) > 0 ? (r.errorMessages || []).join('<br>') : 'N/A'}</p>
</div>`;
    }

    html += '</body></html>';
    this.htmlReport = html;
    return html;
  }

  getResumen() {
    return {
      totalTests: this.totalTests,
      pasos: this.totalPass,
      fallos: this.totalFail,
      saltos: this.totalSkipped,
      tasa: ((this.totalPass / Math.max(1, this.totalTests)) * 100).toFixed(1) + '%',
      errores: this.errors,
    };
  }
}
