import { existsSync, readdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot, resolveInRepo } from '../../engine/lib/paths.mjs';
import { readText, walkMarkdown } from '../../engine/lib/fs.mjs';
import { getRuleAreas, getRuleFiles } from '../../engine/lib/rules-docs.mjs';
import { RULES_DIR } from '../../config/project.mjs';
import { requiredSuites } from '../../config/manifest.mjs';

/**
 * Integridad del propio sistema de reglas: manifiesto de suites (R-CI-7),
 * estructura de `docs/rules`, unicidad de IDs, referencias válidas y enlaces
 * internos. Todo el árbol normativo se inspecciona como dato.
 */

const rulesRoot = resolveInRepo(RULES_DIR);
const RULE_ID_ROW = /^\|\s*(?:\*\*)?((?:R-[A-Z]+|G)-\d+)(?:\*\*)?\s*\|/gm;

export default defineSuite({
  id: 'docs-structure',
  area: 'documentation',
  rules: ['R-CI-7'],
  mode: 'repo',
  run: ({ test, assert }) => {
    test('R-CI-7: existen todas las suites de reglas del manifiesto (no se borra una regla en silencio)', () => {
      const missing = requiredSuites
        .filter((suite) => !existsSync(resolveInRepo('scripts', suite.file)))
        .map((suite) => suite.file);

      assert.deepEqual(
        missing,
        [],
        `Faltan suites de reglas obligatorias del manifiesto (scripts/config/manifest.mjs) (R-CI-7): ${missing.join(', ')}`,
      );
    });

    test('Estructura de áreas: cada área contiene README.md y está enlazada en el índice raíz', () => {
      const areas = getRuleAreas(rulesRoot);
      const rootIndex = readText(join(rulesRoot, 'README.md'));

      for (const area of areas) {
        const directory = join(rulesRoot, area);
        assert.ok(
          existsSync(join(directory, 'README.md')),
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
          const content = readText(path);
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

      for (const path of getRuleFiles(rulesRoot)) {
        const content = readText(path);
        const ids = [...content.matchAll(RULE_ID_ROW)].map((match) => match[1]);

        assert.ok(
          ids.length > 0,
          `${relative(repoRoot, path)}: no define ningún ID de regla o gate en su tabla`,
        );

        for (const id of ids) {
          const previous = definitions.get(id);
          assert.ok(
            !previous,
            `${id} está duplicado: definido en ${relative(repoRoot, path)} y ${previous ? relative(repoRoot, previous) : ''}`,
          );
          definitions.set(id, path);
        }
      }
    });

    test('Referencias de reglas: toda regla referenciada está definida o formalmente retirada', () => {
      const definitions = new Set();
      for (const path of getRuleFiles(rulesRoot)) {
        for (const match of readText(path).matchAll(RULE_ID_ROW)) {
          definitions.add(match[1]);
        }
      }

      // Prefijos e IDs retirados de este repo. Se registran aquí para que las
      // referencias históricas en la documentación sigan siendo válidas.
      const retiredPrefixes = new Set([]);
      const retiredIds = new Set([]);
      const history = join(rulesRoot, '_meta', 'AUDIT-HISTORY.md');

      for (const path of walkMarkdown(rulesRoot).filter((file) => file !== history)) {
        const content = readText(path);
        for (const match of content.matchAll(/\b((?:R-[A-Z]+|G)-\d+)\b/g)) {
          const id = match[1];
          const prefix = id.replace(/-\d+$/, '');
          const isKnown =
            definitions.has(id) ||
            retiredPrefixes.has(prefix) ||
            retiredIds.has(id);
          assert.ok(
            isKnown,
            `${relative(repoRoot, path)}: referencia ${id} que no está definido ni retirado`,
          );
        }
      }
    });

    test('Enlaces locales: todos los enlaces Markdown internos existen y son válidos', () => {
      const history = join(rulesRoot, '_meta', 'AUDIT-HISTORY.md');
      const files = walkMarkdown(rulesRoot).filter((path) => path !== history);

      for (const path of files) {
        const content = readText(path);
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
              `${relative(repoRoot, path)}: enlace local con encoding inválido: ${match[1]}`,
            );
          }
          const destination = resolve(dirname(path), target);
          assert.ok(
            existsSync(destination),
            `${relative(repoRoot, path)}: enlace local roto -> ${match[1]}`,
          );
        }
      }
    });
  },
});
