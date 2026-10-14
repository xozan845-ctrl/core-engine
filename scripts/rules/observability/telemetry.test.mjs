import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { readJson, readText } from '../../engine/lib/fs.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { PACKAGES_DIR, SERVICES } from '../../config/project.mjs';

const prometheusPath = join(repoRoot, 'infra', 'prometheus', 'prometheus.yml');
const alertsPath = join(repoRoot, 'infra', 'prometheus', 'rules', 'alerts.yml');
const grafanaRoot = join(repoRoot, 'infra', 'grafana', 'provisioning');

const serviceDir = (service) => join(repoRoot, PACKAGES_DIR, service);

function portOf(service) {
  const dockerfile = readText(join(serviceDir(service), 'Dockerfile'));
  const match = dockerfile.match(/^EXPOSE\s+(\d+)$/m);
  if (!match) throw new Error(`${service}/Dockerfile no declara EXPOSE`);
  return match[1];
}

function prometheusServiceTargets(config) {
  const section = config.match(
    /- job_name: 'core-engine-servicios'([\s\S]*?)(?=\n\s*- job_name:|\nrule_files:)/,
  )?.[1];
  if (!section) return [];
  return [...section.matchAll(/^\s+- '([^']+)'$/gm)].map((match) => match[1]);
}

function alertBlocks(config) {
  const starts = [...config.matchAll(/^\s{4}- alert:\s*(\S+)\s*$/gm)];
  return starts.map((match, index) => ({
    name: match[1],
    text: config.slice(match.index, starts[index + 1]?.index ?? config.length),
  }));
}

export default defineSuite({
  id: 'observability-telemetry',
  area: 'observability',
  rules: ['R-TM-1', 'R-TM-5', 'R-TM-6', 'R-TM-7'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-TM-1: todos los servicios exponen /metrics mediante MetricsService', () => {
      const problems = [];
      for (const service of SERVICES) {
        const appModule = readText(join(serviceDir(service), 'src', 'app.module.ts'));
        const main = readText(join(serviceDir(service), 'src', 'main.ts'));
        if (!/\bMetricsModule\b/.test(appModule)) {
          problems.push(`${service}: AppModule no importa MetricsModule`);
        }
        if (!/\bMetricsService\b/.test(main)) {
          problems.push(`${service}: main.ts no obtiene MetricsService`);
        }
        if (!/\.get\(['"]\/metrics['"]/.test(main)) {
          problems.push(`${service}: main.ts no registra GET /metrics`);
        }
        if (!/\.type\(['"]text\/plain['"]\)\.send\(await metrics\.texto\(\)\)/.test(main)) {
          problems.push(`${service}: /metrics no responde texto de Prometheus`);
        }
      }
      assert.deepEqual(problems, [], problems.join('\n'));
    });

    test('R-TM-1: Prometheus scrapea exactamente el inventario desplegable', () => {
      const config = readText(prometheusPath);
      const expected = SERVICES.map((service) => {
        const host = service === 'api-gateway' ? 'gateway' : service;
        return `${host}:${portOf(service)}`;
      }).sort();
      const actual = prometheusServiceTargets(config).sort();
      assert.deepEqual(
        actual,
        expected,
        'core-engine-servicios debe incluir una vez cada servicio y su EXPOSE real',
      );
      assert.match(config, /^\s+metrics_path: \/metrics$/m);
    });

    test('R-TM-5: se recolectan Node.js, RabbitMQ, DLQ y recursos del host', () => {
      const metricsService = readText(
        join(repoRoot, PACKAGES_DIR, 'shared', 'src', 'metrics', 'metrics.service.ts'),
      );
      const prometheus = readText(prometheusPath);
      const alerts = readText(alertsPath);
      const compose = readText(join(repoRoot, 'docker-compose.observability.yml'));

      assert.match(metricsService, /collectDefaultMetrics\s*\(/, 'faltan métricas Node.js');
      for (const job of [
        'rabbitmq-exporter',
        'postgres-exporter',
        'node-exporter',
        'cadvisor',
      ]) {
        assert.match(prometheus, new RegExp(`job_name: '${job}'`), `falta el job ${job}`);
        assert.match(compose, new RegExp(`^  ${job}:$`, 'm'), `falta desplegar ${job}`);
      }
      assert.match(compose, /infra\/prometheus\/rules:\/etc\/prometheus\/rules:ro/);
      assert.match(alerts, /queue=~"\.\*\\\\\.dlq\$"/, 'falta seleccionar las colas DLQ');
      assert.match(alerts, /delta\([^\n]+\[\d+m\]\)\s*>\s*0/, 'la DLQ debe alertar por crecimiento');
      assert.match(
        alerts,
        /RabbitMQDLQCrecimientoSostenido[\s\S]*?for:\s*(1[0-9]|[2-9][0-9])m/,
        'el crecimiento de DLQ debe sostenerse antes de alertar',
      );
    });

    test('R-TM-6: datasource, provider y dashboards se aprovisionan como código', () => {
      const datasource = join(grafanaRoot, 'datasources', 'datasources.yml');
      const provider = join(grafanaRoot, 'dashboards', 'dashboards.yml');
      assert.ok(existsSync(datasource), 'falta el datasource provisionado');
      assert.ok(existsSync(provider), 'falta el provider de dashboards');
      assert.match(readText(datasource), /type:\s*prometheus/);
      assert.match(readText(provider), /type:\s*file/);

      const dashboardsDir = join(grafanaRoot, 'dashboards');
      const dashboards = readdirSync(dashboardsDir).filter((name) => name.endsWith('.json'));
      assert.ok(dashboards.length > 0, 'no hay dashboards JSON versionados');
      for (const dashboard of dashboards) {
        const parsed = readJson(join(dashboardsDir, dashboard));
        assert.ok(parsed.title, `${dashboard}: falta title`);
        assert.ok(Array.isArray(parsed.panels), `${dashboard}: falta panels[]`);
      }
    });

    test('R-TM-7: todas las alertas tienen espera, descripción y playbook', () => {
      const blocks = alertBlocks(readText(alertsPath));
      assert.ok(blocks.length > 0, 'no se encontraron alertas Prometheus');
      const problems = [];
      for (const block of blocks) {
        if (!/^\s+for:\s*\S+/m.test(block.text)) problems.push(`${block.name}: falta for`);
        if (!/^\s+description:\s*\S+/m.test(block.text)) {
          problems.push(`${block.name}: falta description`);
        }
        if (!/^\s+runbook_url:\s*\S+/m.test(block.text)) {
          problems.push(`${block.name}: falta runbook_url`);
        }
      }
      assert.deepEqual(problems, [], problems.join('\n'));
      assert.ok(
        existsSync(join(repoRoot, 'docs', 'runbook-observabilidad.md')),
        'el playbook enlazado debe estar versionado',
      );
    });
  },
});
