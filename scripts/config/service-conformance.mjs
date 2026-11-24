/**
 * config/service-conformance.mjs — Contrato de servicio ejecutable (R-MS-9..).
 * ──────────────────────────────────────────────────────────────────────────────
 * Fuente única de las comprobaciones que contrastan `packages/<servicio>/service.json`
 * con la realidad del repositorio: artefactos, compose, esquema, módulos,
 * arranque, observabilidad, eventos y deuda arquitectónica aceptada.
 *
 * Lo consumen:
 *   - `scripts/rules/microservices/service-contract.test.mjs` (suite de reglas),
 *   - `scripts/tools/architecture-report.mjs` (catálogo/matriz generados),
 *   - `scripts/tools/architecture-baseline.mjs` (deuda explícita).
 *
 * Reglas duras (artefactos, identidad, esquema, módulos, arranque, eventos,
 * documentación): no admiten baseline; se corrigen en el PR.
 * Deuda arquitectónica (readiness, env, cross-schema): se congela en
 * `architecture-baseline.json` y sólo puede reducirse (R-AF-2).
 */
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import {
  REPO_ROOT,
  PACKAGES_DIR,
  SERVICE_MANIFESTS,
  SERVICES,
  SERVICE_BY_NAME,
} from './services.mjs';

export const ARCHITECTURE_BASELINE_FILE = 'scripts/config/architecture-baseline.json';

/** Eventos que produce un sistema externo y por tanto no tienen publisher local. */
export const EXTERNAL_EVENTS = Object.freeze(['venta.geolocalizada']);

const PROFILE_RULES = {
  edge: {
    requiredModules: ['auth', 'config', 'errors', 'state', 'logs', 'metrics', 'http', 'policy', 'proxy', 'rate_limit', 'readiness'],
    database: false,
  },
  core: {
    requiredModules: ['auth', 'config', 'db', 'errors', 'state', 'logs', 'metrics', 'http'],
    database: true,
  },
  transactional: {
    requiredModules: ['auth', 'config', 'db', 'errors', 'state', 'logs', 'metrics', 'http', 'rabbit', 'outbox'],
    database: true,
  },
  consumer: {
    requiredModules: ['auth', 'config', 'db', 'errors', 'state', 'logs', 'metrics', 'http', 'rabbit'],
    database: true,
  },
  olap: {
    requiredModules: ['auth', 'config', 'db', 'errors', 'state', 'logs', 'metrics', 'http', 'rabbit'],
    database: true,
  },
};

const SYSTEM_SCHEMAS = new Set(
  SERVICE_MANIFESTS.map((entry) => entry.manifest.spec.schema).filter(Boolean),
);

function readOrEmpty(file) {
  const path = join(REPO_ROOT, file);
  return existsSync(path) ? readFileSync(path, 'utf8') : '';
}

function walk(dir, predicate = () => true) {
  const absolute = join(REPO_ROOT, dir);
  if (!existsSync(absolute)) return [];
  return readdirSync(absolute, { withFileTypes: true }).flatMap((entry) => {
    const rel = join(dir, entry.name);
    if (entry.isDirectory()) return walk(rel, predicate);
    return predicate(rel) ? [rel] : [];
  });
}

function stripTestModules(source) {
  // Los tests unitarios pueden citar esquemas ajenos como fixture; no son
  // acoplamiento de producción. Se corta desde el primer `#[cfg(test)]`.
  return source.split(/\n#\[cfg\(test\)\]/)[0];
}

function serviceFiles(service) {
  return walk(join(PACKAGES_DIR, service, 'src'), (file) => file.endsWith('.rs'));
}

function readServiceSource(service) {
  return serviceFiles(service)
    .map((file) => readOrEmpty(file))
    .join('\n');
}

function composeFile(environment) {
  if (environment === 'prod') return 'docker-compose.yml';
  return `docker-compose.${environment}.yml`;
}

function composeBlock(markdown, name) {
  const lines = markdown.split('\n');
  const start = lines.findIndex((line) => line === `  ${name}:`);
  if (start < 0) return null;
  const block = [];
  for (let i = start + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (/^ {2}[a-z0-9-]+:$/.test(line) || /^[a-z]+:$/.test(line)) break;
    block.push(line);
  }
  return block.join('\n');
}

function hasModule(service, moduleName) {
  const base = join(REPO_ROOT, PACKAGES_DIR, service, 'src', moduleName);
  return existsSync(`${base}.rs`) || existsSync(join(base, 'mod.rs'));
}

function moduleDeclared(service, moduleName) {
  const lib = readOrEmpty(join(PACKAGES_DIR, service, 'src', 'lib.rs'));
  return new RegExp(`(?:pub\\s+)?mod\\s+${moduleName}\\s*;`).test(lib);
}

function cargoPackage(service) {
  const cargo = readOrEmpty(join(PACKAGES_DIR, service, 'Cargo.toml'));
  const name = cargo.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
  const bin = cargo.match(/^\[\[bin\]\][\s\S]*?^name\s*=\s*"([^"]+)"/m)?.[1];
  const publish = /^publish\s*=\s*false$/m.test(cargo);
  return { name, bin, publish };
}

function isUuidLike(value) {
  return /^[a-z][a-z0-9-]*$/.test(value);
}

/**
 * Recolecta todos los hallazgos de conformidad.
 * @returns {{findings: Array<{check: string, key: string, service: string, message: string, remediation: string, baselineable: boolean}>, manifestErrors: string[]}}
 */
export function collectConformance() {
  const findings = [];
  const manifestErrors = [];
  const push = (check, service, key, message, remediation, baselineable = false) => {
    findings.push({ check, service, key, message, remediation, baselineable });
  };

  for (const { dir, manifest } of SERVICE_MANIFESTS) {
    const { metadata, spec } = manifest;
    const profile = PROFILE_RULES[spec.profile];

    // ── Identidad del servicio ────────────────────────────────────────────
    if (metadata.name !== dir) {
      push(
        'identity',
        dir,
        'name',
        `metadata.name="${metadata.name}" no coincide con la carpeta "${dir}"`,
        'Alinea metadata.name con packages/<servicio>.',
      );
    }
    if (!isUuidLike(dir)) {
      push('identity', dir, 'slug', `"${dir}" no es un slug kebab-case`, 'Usa kebab-case.');
    }
    const cargo = cargoPackage(dir);
    if (cargo.name !== dir) {
      push('identity', dir, 'cargo-name', `Cargo.toml name="${cargo.name}" != "${dir}"`, 'El crate debe llamarse como la carpeta.');
    }
    if (cargo.bin !== dir) {
      push('identity', dir, 'cargo-bin', `[[bin]] name="${cargo.bin}" != "${dir}"`, 'El binario debe llamarse como el servicio.');
    }
    if (!cargo.publish) {
      push('identity', dir, 'publish', 'Cargo.toml debe declarar publish = false', 'Los servicios no se publican en crates.io.');
    }

    // ── Artefactos mínimos ────────────────────────────────────────────────
    const requiredFiles = [
      'Cargo.toml',
      'Cargo.lock',
      'Dockerfile',
      'README.md',
      'service.json',
      join('src', 'main.rs'),
      join('src', 'lib.rs'),
    ];
    for (const file of requiredFiles) {
      if (!existsSync(join(REPO_ROOT, PACKAGES_DIR, dir, file))) {
        push('artifacts', dir, file, `falta ${PACKAGES_DIR}/${dir}/${file}`, 'Añade el artefacto que exige R-MS-1/R-CD-3.');
      }
    }

    // ── Dockerfile: binario, puerto y usuario no-root ─────────────────────
    const dockerfile = readOrEmpty(join(PACKAGES_DIR, dir, 'Dockerfile'));
    const expose = dockerfile.match(/^EXPOSE\s+(\d+)$/m)?.[1];
    if (Number(expose) !== spec.port) {
      push('identity', dir, 'expose', `EXPOSE=${expose ?? '(falta)'} != service.json port=${spec.port}`, 'El puerto del Dockerfile y del manifiesto debe ser el mismo.');
    }
    if (!new RegExp(`(?:ENTRYPOINT|CMD)\\s+\\["/usr/local/bin/${dir}"\\]`).test(dockerfile)) {
      push('identity', dir, 'entrypoint', `el Dockerfile no arranca /usr/local/bin/${dir}`, 'Usa el binario del crate como entrypoint.');
    }
    if (!/^USER\s+\d+:\d+$/m.test(dockerfile)) {
      push('identity', dir, 'user', 'el runtime no declara USER no-root', 'R-CD-14 exige usuario numérico no-root.');
    }

    // ── Compose: presencia, puerto y outbox por entorno ───────────────────
    const composeName = spec.composeName ?? dir;
    for (const environment of spec.environments) {
      const file = composeFile(environment);
      const markdown = readOrEmpty(file);
      const block = composeBlock(markdown, composeName);
      if (!block) {
        push('compose', dir, `${environment}:missing`, `no aparece el servicio "${composeName}" en ${file}`, 'Añade el servicio al compose del entorno o retíralo de spec.environments.');
        continue;
      }
      const port = block.match(/^\s+PORT:\s*(\d+)$/m)?.[1];
      if (Number(port) !== spec.port) {
        push('compose', dir, `${environment}:port`, `${file}: PORT=${port ?? '(falta)'} != ${spec.port}`, 'Alinea PORT con service.json.');
      }
      if (spec.outboxTable) {
        const outbox = block.match(/^\s+OUTBOX_TABLA:\s*([^\s#]+)$/m)?.[1];
        if (outbox !== spec.outboxTable) {
          push('compose', dir, `${environment}:outbox`, `${file}: OUTBOX_TABLA=${outbox ?? '(falta)'} != ${spec.outboxTable}`, 'Alinea OUTBOX_TABLA con service.json.');
        }
      }
    }

    // ── Esquema propio ────────────────────────────────────────────────────
    if (profile.database) {
      if (!spec.schema) {
        push('schema', dir, 'missing', 'el perfil exige spec.schema', 'Declara el esquema que posee el servicio (R-DB-1).');
      } else {
        const initSql = readdirSync(join(REPO_ROOT, 'infra', 'db', 'init'))
          .filter((file) => file.endsWith('.sql'))
          .map((file) => readOrEmpty(join('infra', 'db', 'init', file)))
          .join('\n');
        const schemaPattern = new RegExp(`CREATE\\s+SCHEMA(?:\\s+IF\\s+NOT\\s+EXISTS)?\\s+${spec.schema}\\b`, 'i');
        if (!schemaPattern.test(initSql)) {
          push('schema', dir, 'not-created', `no existe CREATE SCHEMA ${spec.schema} en infra/db/init`, 'Añade la migración del esquema.');
        }
      }
    } else if (spec.schema !== null) {
      push('schema', dir, 'unexpected', `el perfil "${spec.profile}" no debería declarar schema`, 'Usa null para servicios sin base de datos.');
    }
    if (spec.outboxTable && spec.schema && !spec.outboxTable.startsWith(`${spec.schema}.`)) {
      push('schema', dir, 'outbox', `outboxTable="${spec.outboxTable}" no pertenece al esquema "${spec.schema}"`, 'El outbox vive en el esquema del servicio (R-AR-3).');
    }

    // ── Módulos exigidos por el perfil ────────────────────────────────────
    for (const moduleName of profile.requiredModules) {
      if (!hasModule(dir, moduleName)) {
        push('modules', dir, moduleName, `falta src/${moduleName}.rs (perfil ${spec.profile})`, 'Crea el módulo o corrige el perfil del manifiesto.');
        continue;
      }
      if (!moduleDeclared(dir, moduleName)) {
        push('modules', dir, `${moduleName}:lib`, `src/lib.rs no declara el módulo ${moduleName}`, 'Declara el módulo en lib.rs para que tests e integración lo usen.');
      }
    }
    if (spec.events.publishes.length > 0 && !hasModule(dir, 'outbox')) {
      push('events', dir, 'outbox', 'publica eventos pero no tiene src/outbox.rs', 'R-AR-3: los eventos salen por outbox transaccional.');
    }

    // ── Arranque: fail-fast, bind y apagado ordenado ──────────────────────
    const main = readOrEmpty(join(PACKAGES_DIR, dir, 'src', 'main.rs'));
    if (!/Config::from_env\s*\(/.test(main)) {
      push('startup', dir, 'config', 'main.rs no resuelve la configuración con Config::from_env()', 'R-MS-5: configuración fail-fast en config.rs.');
    }
    if (!/process::exit\s*\(\s*1\s*\)/.test(main)) {
      push('startup', dir, 'fail-fast', 'main.rs no termina con exit(1) ante configuración inválida', 'R-MS-5: si falta una variable crítica el proceso no arranca.');
    }
    if (!/with_graceful_shutdown\s*\(/.test(main)) {
      push('startup', dir, 'shutdown', 'main.rs no usa with_graceful_shutdown', 'R-MS-6: apagado ordenado ante SIGINT/SIGTERM.');
    }
    if (!/TcpListener::bind\s*\(/.test(main) || !/0\.0\.0\.0/.test(main)) {
      push('startup', dir, 'bind', 'main.rs no enlaza 0.0.0.0', 'R-MS-6/12-factor: el proceso escucha en 0.0.0.0:<PORT>.');
    }

    // ── Stateless: el contenedor no persiste estado de negocio ────────────
    for (const file of serviceFiles(dir)) {
      const source = stripTestModules(readOrEmpty(file));
      if (/(?:std::fs|tokio::fs)::(?:write|create|remove|rename|copy|append)/.test(source)) {
        push(
          'stateless',
          dir,
          relative(join(PACKAGES_DIR, dir, 'src'), file).replaceAll('\\', '/'),
          'escribe estado en el filesystem del contenedor',
          'R-MS-12: usa PostgreSQL, RabbitMQ u object storage; el contenedor es efímero.',
        );
      }
    }

    // ── Observabilidad: /health, /metrics, RED y request-id ───────────────
    const http = readOrEmpty(join(PACKAGES_DIR, dir, 'src', 'http', 'mod.rs'));
    for (const route of ['/health', '/metrics']) {
      if (!http.includes(`.route("${route}"`)) {
        push('observability', dir, route, `http/mod.rs no expone ${route}`, 'R-TM-1/R-MS-11: todo servicio expone salud y métricas.');
      }
    }
    const metrics = readOrEmpty(join(PACKAGES_DIR, dir, 'src', 'metrics.rs'));
    for (const metric of ['core_engine_http_requests_total', 'core_engine_http_request_duration_seconds']) {
      if (!metrics.includes(metric)) {
        push('observability', dir, metric, `metrics.rs no registra ${metric}`, 'R-TM-4: métricas RED obligatorias.');
      }
    }
    const logs = readOrEmpty(join(PACKAGES_DIR, dir, 'src', 'logs.rs'));
    if (!/serde_json|json!/.test(logs)) {
      push('observability', dir, 'json-logs', 'logs.rs no serializa JSON', 'R-OB-1: logs estructurados en JSON.');
    }
    if (!readServiceSource(dir).includes('x-request-id')) {
      push('observability', dir, 'request-id', 'el servicio no propaga x-request-id', 'R-OB-2: correlación HTTP y outbox/RabbitMQ.');
    }

    // ── Readiness real (deuda congelada) ──────────────────────────────────
    if (profile.database) {
      const ready = http.includes('.route("/ready"');
      if (!ready) {
        push(
          'readiness',
          dir,
          'ready',
          'servicio con base de datos sin /ready (sólo /health de liveness)',
          'Expón /ready con 503 si PostgreSQL/RabbitMQ no están disponibles; /health se queda como liveness.',
          true,
        );
      }
    }

    // ── Configuración: env sólo en config.rs/db.rs ────────────────────────
    for (const file of serviceFiles(dir)) {
      const rel = relative(join(PACKAGES_DIR, dir, 'src'), file).replaceAll('\\', '/');
      if (rel === 'config.rs' || rel === 'db.rs') continue;
      const source = stripTestModules(readOrEmpty(file));
      if (/(?:std::)?env::var\s*\(/.test(source)) {
        push(
          'env',
          dir,
          rel,
          `${rel} lee variables de entorno fuera de config.rs/db.rs`,
          'R-MS-5: resuelve la configuración una vez en config.rs y pásala por AppState.',
          true,
        );
      }
    }

    // ── Aislamiento de datos: sin SQL a esquemas ajenos ───────────────────
    if (spec.schema) {
      const foreign = new Set();
      const pattern = /(FROM|JOIN|INTO|UPDATE|DELETE\s+FROM)\s+([a-z_]+)\.[a-z_]+/g;
      for (const file of serviceFiles(dir)) {
        const source = stripTestModules(readOrEmpty(file));
        for (const match of source.matchAll(pattern)) {
          const schema = match[2];
          if (SYSTEM_SCHEMAS.has(schema) && schema !== spec.schema) foreign.add(schema);
        }
      }
      for (const schema of [...foreign].sort()) {
        push(
          'cross-schema',
          dir,
          schema,
          `consulta/actualiza el esquema ajeno "${schema}"`,
          `R-DB-1/R-MS-16: consume ${schema} por API interna o por eventos y proyecta en "${spec.schema}".`,
          true,
        );
      }
    }

    // ── Eventos declarados vs. código ─────────────────────────────────────
    const source = readServiceSource(dir);
    for (const event of spec.events.publishes) {
      if (!source.includes(event)) {
        push('events', dir, `pub:${event}`, `declara publicar "${event}" pero no aparece en src/`, 'Declara el evento en el outbox o retíralo del manifiesto.');
      }
    }
    for (const event of spec.events.consumes) {
      if (!source.includes(event)) {
        push('events', dir, `con:${event}`, `declara consumir "${event}" pero no aparece en src/`, 'Añade la routing key al consumidor o retíralo del manifiesto.');
      }
    }

    // ── Dependencias síncronas declaradas ─────────────────────────────────
    for (const dependency of spec.dependencies.sync) {
      if (!SERVICE_BY_NAME[dependency]) {
        push('dependencies', dir, dependency, `depende de "${dependency}", que no existe`, 'Corrige el nombre del servicio dependiente.');
      }
    }

    // ── Documentación y contrato ──────────────────────────────────────────
    const readme = readOrEmpty(join(PACKAGES_DIR, dir, 'README.md'));
    if (readme.length < 500 || !/^##\s+/m.test(readme)) {
      push('docs', dir, 'readme', 'README.md ausente o sin secciones', 'R-MS-8: documenta bounded context, capacidades, tablas y variables.');
    }
    if (spec.publicApi || spec.internalApi) {
      if (!spec.docs.openapi) {
        push('docs', dir, 'openapi', 'servicio con API sin spec OpenAPI declarada', 'R-DO-6: publica el contrato y sincronízalo con swagger:export.');
      } else if (spec.docs.openapi.endsWith('.json')) {
        if (!existsSync(join(REPO_ROOT, spec.docs.openapi))) {
          push('docs', dir, 'openapi-file', `no existe ${spec.docs.openapi}`, 'Genera el contrato con npm run swagger:export.');
        }
      } else {
        for (const file of ['public.openapi.json', 'internal.openapi.json']) {
          if (!existsSync(join(REPO_ROOT, spec.docs.openapi, file))) {
            push('docs', dir, `openapi:${file}`, `falta ${spec.docs.openapi}/${file}`, 'Genera el contrato con npm run swagger:export.');
          }
        }
      }
    }
  }

  // ── Inventario: cada paquete con Dockerfile tiene manifiesto ────────────
  const packagesRoot = join(REPO_ROOT, PACKAGES_DIR);
  const deployable = readdirSync(packagesRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && existsSync(join(packagesRoot, entry.name, 'Dockerfile')))
    .map((entry) => entry.name);
  for (const dir of deployable) {
    if (!SERVICES.includes(dir)) {
      manifestErrors.push(`${PACKAGES_DIR}/${dir}: tiene Dockerfile pero no entra en el catálogo`);
    }
    if (!existsSync(join(packagesRoot, dir, 'service.json'))) {
      manifestErrors.push(`${PACKAGES_DIR}/${dir}/service.json: falta el manifiesto (R-MS-9)`);
    }
  }

  // ── Eventos: todo consume tiene publisher local o externo ───────────────
  const publishers = new Map();
  for (const { manifest } of SERVICE_MANIFESTS) {
    for (const event of manifest.spec.events.publishes) {
      if (!publishers.has(event)) publishers.set(event, []);
      publishers.get(event).push(manifest.metadata.name);
    }
  }
  for (const { manifest } of SERVICE_MANIFESTS) {
    for (const event of manifest.spec.events.consumes) {
      if (!publishers.has(event) && !EXTERNAL_EVENTS.includes(event)) {
        push(
          'events',
          manifest.metadata.name,
          `orphan:${event}`,
          `consume "${event}" pero ningún servicio lo publica`,
          'Declara el publisher en su manifiesto o añade el evento a EXTERNAL_EVENTS si lo produce un sistema externo.',
        );
      }
    }
  }
  for (const [event, names] of publishers) {
    if (names.length > 1) {
      push(
        'events',
        names.join(','),
        `multi:${event}`,
        `"${event}" lo publican varios servicios: ${names.join(', ')}`,
        'Un evento tiene un solo dueño; los demás deben consumirlo o emitir otro tipo.',
      );
    }
  }

  return { findings, manifestErrors };
}

/** Carga el baseline de deuda arquitectónica. */
export function loadArchitectureBaseline() {
  const path = join(REPO_ROOT, ARCHITECTURE_BASELINE_FILE);
  if (!existsSync(path)) return { debt: [] };
  const parsed = JSON.parse(readFileSync(path, 'utf8'));
  if (!Array.isArray(parsed.debt)) {
    throw new Error(`${ARCHITECTURE_BASELINE_FILE}: falta el array "debt"`);
  }
  return parsed;
}

/**
 * Separa hallazgos nuevos (fallan) de deuda aceptada (pasa) y detecta
 * entradas obsoletas (deben retirarse para que el baseline sólo baje).
 */
export function partitionFindings(findings, baseline) {
  const accepted = new Set(baseline.debt ?? []);
  const newFindings = [];
  const debt = [];
  const matched = new Set();

  for (const finding of findings) {
    if (!finding.baselineable) {
      newFindings.push(finding);
      continue;
    }
    const key = findingKey(finding);
    if (accepted.has(key)) {
      debt.push(finding);
      matched.add(key);
    } else {
      newFindings.push(finding);
    }
  }

  const obsolete = [...accepted].filter((key) => !matched.has(key));
  return { newFindings, debt, obsolete };
}

export function findingKey(finding) {
  return `${finding.check}:${finding.service}:${finding.key}`;
}

function table(headers, rows) {
  const lines = [`| ${headers.join(' | ')} |`, `| ${headers.map(() => '---').join(' | ')} |`];
  for (const row of rows) lines.push(`| ${row.join(' | ')} |`);
  return lines.join('\n');
}

/** Catálogo de servicios en Markdown, generado desde los manifiestos. */
export function renderCatalogMarkdown() {
  const rows = SERVICE_MANIFESTS.map(({ manifest }) => {
    const { metadata, spec } = manifest;
    return [
      `[${metadata.name}](../../packages/${metadata.name}/README.md)`,
      metadata.boundedContext,
      spec.profile,
      String(spec.port),
      spec.schema ?? '—',
      spec.capabilities.length > 0 ? spec.capabilities.join(', ') : '—',
      spec.environments.join(', '),
      spec.events.publishes.length,
      spec.events.consumes.length,
      metadata.owner,
      metadata.lifecycle,
    ];
  });
  return `${[
    '<!-- GENERADO por scripts/tools/architecture-report.mjs. No editar a mano. -->',
    '# Catálogo de servicios',
    '',
    'Fuente de verdad: `packages/<servicio>/service.json` (R-MS-9). Este archivo se',
    'regenera con `npm run architecture:report`; CI falla si está desactualizado.',
    '',
    table(
      ['Servicio', 'Bounded context', 'Perfil', 'Puerto', 'Esquema', 'Capacidades', 'Entornos', 'Publica', 'Consume', 'Owner', 'Lifecycle'],
      rows,
    ),
    '',
    '## Perfiles',
    '',
    '| Perfil | Significado |',
    '|---|---|',
    '| `edge` | Perímetro: JWT, políticas, proxy, rate limit. Sin base de datos. |',
    '| `core` | Servicio síncrono con esquema propio, sin broker. |',
    '| `transactional` | Publica eventos de dominio: exige outbox y RabbitMQ. |',
    '| `consumer` | Consume eventos; no publica ni mantiene outbox propio. |',
    '| `olap` | Analítica/proyecciones; no muta el dominio transaccional. |',
    '',
  ].join('\n')}`;
}

/** Matriz de eventos en Markdown, generada desde los manifiestos. */
export function renderEventMatrixMarkdown() {
  const events = new Map();
  for (const { manifest } of SERVICE_MANIFESTS) {
    for (const event of manifest.spec.events.publishes) {
      if (!events.has(event)) events.set(event, { publishers: [], consumers: [] });
      events.get(event).publishers.push(manifest.metadata.name);
    }
    for (const event of manifest.spec.events.consumes) {
      if (!events.has(event)) events.set(event, { publishers: [], consumers: [] });
      events.get(event).consumers.push(manifest.metadata.name);
    }
  }
  for (const event of EXTERNAL_EVENTS) {
    if (events.has(event)) events.get(event).publishers.push('sistema externo');
  }
  const rows = [...events.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([event, { publishers, consumers }]) => [
      `\`${event}\``,
      publishers.length > 0 ? publishers.join(', ') : '**huérfano**',
      consumers.length > 0 ? consumers.join(', ') : '—',
    ]);
  return `${[
    '<!-- GENERADO por scripts/tools/architecture-report.mjs. No editar a mano. -->',
    '# Matriz de eventos',
    '',
    'Topic: `core-engine.events`. Envelope: `tipo`, `event_id`, `data` y',
    'metadatos de correlación (`x-request-id`). Fuente: `service.json` de cada',
    'servicio; se regenera con `npm run architecture:report`.',
    '',
    table(['Evento', 'Publica', 'Consume'], rows),
    '',
    '> Un evento sin publisher local puede ser externo (`venta.geolocalizada`) o',
    '> deuda: la suite `services-contract` falla si un consumidor declara un evento',
    '> que nadie publica y no está en `EXTERNAL_EVENTS`.',
    '',
  ].join('\n')}`;
}

/** Deuda arquitectónica en Markdown, derivada del baseline. */
export function renderDebtMarkdown() {
  const baseline = loadArchitectureBaseline();
  const byCheck = new Map();
  for (const key of baseline.debt ?? []) {
    const [check, ...rest] = key.split(':');
    if (!byCheck.has(check)) byCheck.set(check, []);
    byCheck.get(check).push(rest.join(':'));
  }
  const descriptions = {
    readiness: 'Servicios con base de datos que aún no exponen `/ready` (503 si una dependencia no está lista).',
    env: 'Archivos que aún leen variables de entorno fuera de `config.rs`/`db.rs` (R-MS-5).',
    'cross-schema': 'Servicios que aún consultan o escriben esquemas ajenos (R-DB-1/R-MS-16).',
  };
  const sections = [...byCheck.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([check, services]) => {
      const rows = services
        .sort()
        .map((entry) => {
          const [service, ...detail] = entry.split(':');
          return `| \`${service}\` | ${detail.length > 0 ? `\`${detail.join(':')}\`` : '—'} |`;
        });
      return `## ${check}\n\n${descriptions[check] ?? 'Deuda arquitectónica aceptada.'}\n\n| Servicio | Detalle |\n|---|---|\n${rows.join('\n')}`;
    });
  return `${[
    '<!-- GENERADO por scripts/tools/architecture-report.mjs. No editar a mano. -->',
    '# Deuda arquitectónica aceptada',
    '',
    'El baseline `scripts/config/architecture-baseline.json` congela la deuda',
    'existente (R-AF-2). **No puede crecer**: `npm run baseline:guard` rechaza una',
    'entrada nueva y la suite `services-contract` falla si una entrada ya no',
    'corresponde a un hallazgo real (debe retirarse en el mismo PR que la corrige).',
    '',
    `Total: **${baseline.debt?.length ?? 0}** entradas.`,
    '',
    sections.join('\n\n'),
    '',
  ].join('\n')}`;
}
