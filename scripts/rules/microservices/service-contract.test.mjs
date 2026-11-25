import { defineSuite } from '../../engine/suite.mjs';
import {
  collectConformance,
  loadArchitectureBaseline,
  partitionFindings,
} from '../../config/service-conformance.mjs';
import { SERVICE_MANIFESTS, SERVICES } from '../../config/project.mjs';

/**
 * Contrato de servicio (R-MS-9..R-MS-16) y fitness functions de arquitectura
 * (R-AF-1..R-AF-4). La lógica vive en `config/service-conformance.mjs` para que
 * la suite, el catálogo generado y el baseline compartan una sola definición.
 */

function format(findings) {
  return findings
    .map((finding) => `- [${finding.check}] ${finding.service}: ${finding.message}\n  → ${finding.remediation}`)
    .join('\n');
}

function assertCheck(assert, findings, check, message) {
  const selected = findings.filter((finding) => finding.check === check);
  assert.deepEqual(selected.map((finding) => finding.message), [], `${message}\n${format(selected)}`);
}

export default defineSuite({
  id: 'services-contract',
  area: 'microservice',
  rules: [
    'R-MS-1',
    'R-MS-8',
    'R-MS-9',
    'R-MS-10',
    'R-MS-11',
    'R-MS-12',
    'R-MS-13',
    'R-MS-14',
    'R-MS-15',
    'R-MS-16',
    'R-AF-1',
    'R-AF-2',
    'R-AF-3',
    'R-AF-4',
    'R-DB-1',
    'R-CD-3',
    'R-CD-4',
    'R-DO-6',
    'R-TM-1',
    'R-TM-4',
    'R-OB-1',
  ],
  mode: 'repo',
  run: ({ test, assert }) => {
    const { findings, manifestErrors } = collectConformance();
    const baseline = loadArchitectureBaseline();
    const { newFindings, obsolete } = partitionFindings(findings, baseline);

    test('R-MS-9: todo paquete desplegable declara un manifiesto válido', () => {
      assert.deepEqual(manifestErrors, [], manifestErrors.join('\n'));
      assert.equal(
        new Set(SERVICES).size,
        SERVICES.length,
        'el catálogo derivado no admite servicios duplicados',
      );
      assert.deepEqual(
        SERVICES,
        SERVICE_MANIFESTS.map((entry) => entry.dir),
        'SERVICES debe derivarse exactamente de los manifiestos',
      );
    });

    test('R-MS-1/R-CD-3: cada servicio tiene los artefactos del crate y la imagen', () => {
      assertCheck(assert, findings, 'artifacts', 'Faltan artefactos obligatorios del servicio:');
    });

    test('R-MS-9/R-CD-4: nombre, binario, puerto y entrypoint son coherentes', () => {
      assertCheck(assert, findings, 'identity', 'La identidad del servicio no es coherente:');
    });

    test('R-MS-9/R-CD-3: el compose declara puerto y outbox del manifiesto', () => {
      assertCheck(assert, findings, 'compose', 'El compose no coincide con el manifiesto:');
    });

    test('R-DB-1/R-MS-16: cada servicio declara y crea su propio esquema', () => {
      assertCheck(assert, findings, 'schema', 'El esquema del servicio no está declarado o migrado:');
    });

    test('R-MS-10: los módulos exigidos por el perfil existen y se exportan', () => {
      assertCheck(assert, findings, 'modules', 'Faltan módulos o exports exigidos por el perfil:');
    });

    test('R-MS-5/R-MS-6: arranque fail-fast, bind y apagado ordenado', () => {
      assertCheck(assert, findings, 'startup', 'El ciclo de vida del proceso no cumple el contrato:');
    });

    test('R-MS-12: el servicio no persiste estado en el filesystem', () => {
      assertCheck(assert, findings, 'stateless', 'El servicio escribe estado local:');
    });

    test('R-MS-11/R-TM-1/R-TM-4/R-OB-1: salud, métricas RED y logs JSON', () => {
      assertCheck(assert, findings, 'observability', 'La superficie de observabilidad no cumple el contrato:');
    });

    test('R-MS-14: los eventos declarados existen en el código y tienen dueño', () => {
      assertCheck(assert, findings, 'events', 'El contrato de eventos está roto:');
    });

    test('R-MS-9: las dependencias síncronas apuntan a servicios del catálogo', () => {
      assertCheck(assert, findings, 'dependencies', 'Hay dependencias síncronas inexistentes:');
    });

    test('R-MS-8/R-DO-6: README y contrato OpenAPI presentes', () => {
      assertCheck(assert, findings, 'docs', 'Documentación o contrato incompletos:');
    });

    test('R-DB-1/R-MS-16: cero escrituras directas a esquemas ajenos', () => {
      assertCheck(
        assert,
        findings,
        'cross-schema-write',
        'Hay escrituras directas a tablas de otro servicio:',
      );
    });

    test('R-AF-2: la deuda arquitectónica aceptada no crece ni queda obsoleta', () => {
      const newDebt = newFindings.filter((finding) => finding.baselineable);
      assert.deepEqual(
        newDebt.map((finding) => finding.message),
        [],
        `Deuda arquitectónica nueva no permitida (R-AF-2). Corrige el hallazgo o, si es deuda consciente, justifícala en el ADR y en scripts/config/architecture-baseline.json:\n${format(newDebt)}`,
      );
      assert.deepEqual(
        obsolete,
        [],
        `El baseline contiene entradas que ya no corresponden a ningún hallazgo; retíralas en este PR para que la deuda sólo baje:\n${obsolete.join('\n')}`,
      );
    });

    test('R-AF-3: los hallazgos duros no se pueden congelar en el baseline', () => {
      // El baseline sólo admite checks de deuda; cualquier prefijo desconocido
      // es un intento de congelar una regla dura.
      const allowed = new Set(['readiness', 'env', 'cross-schema']);
      const invalid = baseline.debt.filter((key) => !allowed.has(key.split(':')[0]));
      assert.deepEqual(
        invalid,
        [],
        `El baseline sólo admite readiness/env/cross-schema (R-AF-2):\n${invalid.join('\n')}`,
      );
    });
  },
});
