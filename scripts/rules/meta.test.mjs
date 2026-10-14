import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import {
  getRuleAreas,
  getRuleFiles,
  repositoryRoot,
  rulesRoot,
  walkMarkdown,
} from './helpers/repo.mjs';
import { REQUIRED_RULE_SUITES } from '../project.config.mjs';

test('R-CI-9: existen todas las suites de reglas del manifiesto (no se borra una regla en silencio)', () => {
  const suitesDir = join(repositoryRoot, 'scripts', 'rules');
  const present = new Set(
    readdirSync(suitesDir).filter((file) => file.endsWith('.test.mjs')),
  );

  const missing = REQUIRED_RULE_SUITES.filter((file) => !present.has(file));
  assert.deepEqual(
    missing,
    [],
    `Faltan suites de reglas obligatorias del manifiesto (scripts/project.config.mjs) (R-CI-9): ${missing.join(', ')}`,
  );
});

test('Estructura de áreas: cada área contiene README.md y está enlazada en el índice raíz', () => {
  const areas = getRuleAreas();
  const rootIndex = readFileSync(join(rulesRoot, 'README.md'), 'utf8');

  for (const area of areas) {
    const directory = join(rulesRoot, area);
    const readme = join(directory, 'README.md');
    assert.ok(
      existsSync(readme),
      `El área ${area}/ no tiene archivo README.md requerido`,
    );
    assert.ok(
      rootIndex.includes(`./${area}/README.md`),
      `El índice docs/rules/README.md no enlaza el área ./${area}/README.md`,
    );

    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      if (
        !entry.isFile() ||
        !entry.name.endsWith('.md') ||
        entry.name === 'README.md'
      ) {
        continue;
      }
      assert.match(
        entry.name,
        /^\d{2}-.+\.md$/,
        `${area}/${entry.name}: el nombre del archivo debe seguir la convención NN-tema.md`,
      );

      const path = join(directory, entry.name);
      const content = readFileSync(path, 'utf8');
      const number = entry.name.slice(0, 2);
      assert.ok(
        content.startsWith(`# ${number} — `),
        `${area}/${entry.name}: el título debe comenzar con '# ${number} — '`,
      );
      assert.ok(
        content.includes('| ID | Regla |'),
        `${area}/${entry.name}: falta la tabla normativa '| ID | Regla |'`,
      );
    }
  }
});

test('Unicidad de IDs: cada ID de regla o gate se define exactamente una vez', () => {
  const definitions = new Map();
  const ruleFiles = getRuleFiles();

  for (const path of ruleFiles) {
    const content = readFileSync(path, 'utf8');
    const ids = [
      ...content.matchAll(
        /^\|\s*(?:\*\*)?((?:R-[A-Z]+|G)-\d+)(?:\*\*)?\s*\|/gm,
      ),
    ].map((match) => match[1]);

    assert.ok(
      ids.length > 0,
      `${relative(repositoryRoot, path)}: no define ningún ID de regla o gate en su tabla`,
    );

    for (const id of ids) {
      const previous = definitions.get(id);
      assert.ok(
        !previous,
        `${id} está duplicado: definido en ${relative(repositoryRoot, path)} y ${previous ? relative(repositoryRoot, previous) : ''}`,
      );
      definitions.set(id, path);
    }
  }
});

test('Referencias de reglas: toda regla referenciada está definida o formalmente retirada', () => {
  const definitions = new Set();
  for (const path of getRuleFiles()) {
    const content = readFileSync(path, 'utf8');
    for (const match of content.matchAll(
      /^\|\s*(?:\*\*)?((?:R-[A-Z]+|G)-\d+)(?:\*\*)?\s*\|/gm,
    )) {
      definitions.add(match[1]);
    }
  }

  // Prefijos e IDs retirados de este repo. Se registran aquí para que las
  // referencias históricas en la documentación sigan siendo válidas.
  const retiredPrefixes = new Set([]);
  const retiredIds = new Set([]);
  const history = join(rulesRoot, '_meta', 'AUDIT-HISTORY.md');

  for (const path of walkMarkdown(rulesRoot).filter((file) => file !== history)) {
    const content = readFileSync(path, 'utf8');
    for (const match of content.matchAll(/\b((?:R-[A-Z]+|G)-\d+)\b/g)) {
      const id = match[1];
      const prefix = id.replace(/-\d+$/, '');
      const isKnown =
        definitions.has(id) ||
        retiredPrefixes.has(prefix) ||
        retiredIds.has(id);
      assert.ok(
        isKnown,
        `${relative(repositoryRoot, path)}: referencia ${id} que no está definido ni retirado`,
      );
    }
  }
});

test('Enlaces locales: todos los enlaces Markdown internos existen y son válidos', () => {
  const history = join(rulesRoot, '_meta', 'AUDIT-HISTORY.md');
  const files = walkMarkdown(rulesRoot).filter((path) => path !== history);

  for (const path of files) {
    const content = readFileSync(path, 'utf8');
    const links = content.matchAll(/!?(?:\[[^\]]*\])\(([^)]+)\)/g);
    for (const match of links) {
      let target = match[1].trim();
      if (
        !target ||
        target.startsWith('#') ||
        /^[a-z][a-z\d+.-]*:/i.test(target)
      ) {
        continue;
      }
      if (target.startsWith('<') && target.endsWith('>')) {
        target = target.slice(1, -1);
      }
      target = target.split('#', 1)[0].split('?', 1)[0];
      try {
        target = decodeURIComponent(target);
      } catch {
        assert.fail(
          `${relative(repositoryRoot, path)}: enlace local con encoding inválido: ${match[1]}`,
        );
      }
      const destination = resolve(dirname(path), target);
      assert.ok(
        existsSync(destination),
        `${relative(repositoryRoot, path)}: enlace local roto -> ${match[1]}`,
      );
    }
  }
});
