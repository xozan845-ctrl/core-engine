import test from 'node:test';
import assert from 'node:assert/strict';
import {
  collectConformance,
  findingKey,
  partitionFindings,
  renderCatalogMarkdown,
  renderEventMatrixMarkdown,
} from '../config/service-conformance.mjs';
import { validateManifestShape } from '../config/services.mjs';
import { SERVICES } from '../config/project.mjs';

test('architecture: el manifiesto mínimo exige perfil, puerto, esquema y SLO', () => {
  const errors = validateManifestShape({
    apiVersion: 'core-engine.io/v1',
    kind: 'Service',
    metadata: {
      name: 'a',
      boundedContext: 'a',
      description: 'descripción suficientemente larga',
      owner: 'core-platform',
      lifecycle: 'production',
      tags: ['rust'],
    },
    spec: {
      profile: 'inventado',
      runtime: 'rust',
      port: 0,
      schema: 'a',
      outboxTable: null,
      capabilities: [],
      publicApi: true,
      internalApi: true,
      environments: ['prod'],
      dependencies: { sync: [] },
      events: { publishes: [], consumes: [] },
      slo: { availability: 99.5, latencyP95Ms: 1500 },
      docs: { readme: 'README.md', openapi: null },
    },
    quality: { coverage: { lines: 120, functions: 1 } },
  });
  assert.match(errors.join('\n'), /spec\.profile/);
  assert.match(errors.join('\n'), /spec\.port/);
  assert.match(errors.join('\n'), /quality\.coverage/);
});

test('architecture: el baseline separa deuda aceptada, deuda nueva y obsoleta', () => {
  const findings = [
    { check: 'readiness', service: 'a', key: 'ready', message: 'a', remediation: '', baselineable: true },
    { check: 'readiness', service: 'b', key: 'ready', message: 'b', remediation: '', baselineable: true },
    { check: 'artifacts', service: 'a', key: 'Dockerfile', message: 'x', remediation: '', baselineable: false },
  ];
  const { newFindings, debt, obsolete } = partitionFindings(findings, {
    debt: [findingKey(findings[0]), 'readiness:z:ready'],
  });
  assert.deepEqual(debt.map((finding) => finding.service), ['a']);
  assert.deepEqual(newFindings.map((finding) => finding.service).sort(), ['a', 'b']);
  assert.deepEqual(obsolete, ['readiness:z:ready']);
});

test('architecture: el catálogo generado cubre todos los servicios', () => {
  const markdown = renderCatalogMarkdown();
  assert.match(markdown, /GENERADO/);
  for (const service of SERVICES) {
    assert.ok(markdown.includes(`[${service}]`), `falta ${service} en el catálogo`);
  }
});

test('architecture: la matriz de eventos enlaza publicadores y consumidores', () => {
  const markdown = renderEventMatrixMarkdown();
  assert.match(markdown, /`order\.created`/);
  assert.match(markdown, /orders-service/);
  assert.match(markdown, /inventory-service/);
});

test('architecture: la conformidad actual no tiene hallazgos duros', () => {
  const { findings, manifestErrors } = collectConformance();
  assert.deepEqual(manifestErrors, []);
  assert.deepEqual(
    findings.filter((finding) => !finding.baselineable).map((finding) => finding.message),
    [],
  );
});
