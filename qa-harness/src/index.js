#!/usr/bin/env node
/**
 * Core Engine · Core Engine — QA Harness
 * Testing agresivo y automatizado para el proyecto.
 * Reporte ultra detallado: JSON, Markdown, HTML.
 * Evaluación de control y calidad.
 */
import { Configuracion } from './config.js';
import { Runner } from './runner/runner.js';
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);

// ── Cargar configuración ────────────────────────────────────────────────
const config = new Configuracion();

// ── Parser de argumentos CLI ────────────────────────────────────────────
const args = process.argv.slice(2);
const options = {
  baseUrl: config.baseUrl,
  suites: config.suites,
  format: config.format,
  loadLevel: config.loadLevel,
  concurrency: config.concurrency,
  fuzzRounds: config.fuzzRounds,
  cleanup: config.cleanup,
  outputDir: './reports',
  help: false,
};

for (let i = 0; i < args.length; i++) {
  switch (args[i]) {
    case '--base-url':
    case '-u':
      options.baseUrl = args[++i];
      break;
    case '--suites':
    case '-s':
      options.suites = args[++i].split(',');
      break;
    case '--format':
    case '-f':
      options.format = args[++i];
      break;
    case '--load':
    case '-l':
      options.loadLevel = args[++i];
      break;
    case '--concurrency':
    case '-c':
      options.concurrency = parseInt(args[++i], 10);
      break;
    case '--fuzz':
      options.fuzzRounds = parseInt(args[++i], 10);
      break;
    case '--no-cleanup':
      options.cleanup = false;
      break;
    case '--output':
    case '-o':
      options.outputDir = args[++i];
      break;
    case '--help':
    case '-h':
      options.help = true;
      break;
  }
}

// Hacer efectivo --suites en el Runner
config.suites = options.suites ?? config.suites ?? [];

if (options.help) {
  console.log(`
Core Engine QA Harness — Testing agresivo automatizado

Uso:
  node src/index.js [opciones]

Opciones:
  -u, --base-url <url>      URL base del API Gateway (default: http://localhost:8080)
  -s, --suites <lista>      Suites a ejecutar, separadas por coma (default: todas)
  -f, --format <fmt>        Formato de reporte: html, md, json, all (default: all)
  -l, --load <nivel>        Nivel de carga: agresivo, moderado, ligero (default: agresivo)
  -c, --concurrency <n>     Concurrencia para tests de carga (default: 50)
  --fuzz <n>                Rondas de fuzzing (default: 10)
  --no-cleanup              No limpiar datos de prueba
  -o, --output <dir>        Directorio de salida para reportes (default: ./reports)
  -h, --help                Mostrar esta ayuda

Suites disponibles:
  00-preflight     Verificación pre-ejecución (health, puertos, BD, RabbitMQ)
  01-auth          Autenticación y sesiones (RF-01, RF-02, RF-03)
  02-role-access   Control de acceso y roles (ACL Tabla 21)
  03-catalog       Catálogo CRUD y reglas (RN-01, RN-02, RN-08)
  04-stores        Tiendas y ofertas (RN-02, RN-03)
  05-orders        Órdenes y saga (TC-03, TC-04, estado machine)
  06-cart          Carrito y checkout (RN-05)
  07-logistics     Logística y envíos
  08-commissions   Comisiones y liquidaciones (RN-04, RN-06, RN-07)
  09-finanzas      Finanzas, contabilidad, régimen fiscal NIC
  10-admin         Reportes admin e inventario
  11-fuzzing       Fuzzing y validación agresiva
  12-concurrency   Concurrencia y race conditions
  13-load          Carga y rendimiento
  14-queue         Outbox, DLQ, RabbitMQ
  15-static        Análisis estático (lint, build, tests, debt)

Ejemplos:
  node src/index.js --suites 01-auth,02-role-access --format html
  node src/index.js --load agresivo --concurrency 100 --fuzz 20
  node src/index.js --no-cleanup --output ./my-reports
`);
  process.exit(0);
}

// ── Ejecutar runner ─────────────────────────────────────────────────────
async function main() {
  console.log('╔══════════════════════════════════════════════════════════════╗');
  console.log('║     Core Engine · Core Engine — QA Harness v1.0.0             ║');
  console.log('║     Testing agresivo automatizado con reporte ultra detallado║');
  console.log('╚══════════════════════════════════════════════════════════════╝');

  const runner = new Runner(config);

  try {
    await runner.runAll();
    const reports = runner.generateReports();

    // Guardar reportes
    const outputDir = join(process.cwd(), options.outputDir);
    if (!existsSync(outputDir)) mkdirSync(outputDir, { recursive: true });
    
    writeFileSync(join(outputDir, 'report.html'), reports.html);
    writeFileSync(join(outputDir, 'report.md'), reports.markdown);
    writeFileSync(join(outputDir, 'report.json'), reports.json);

    console.log(`\n📄 Reportes guardados en: ${outputDir}`);
    console.log(`   - report.html (reporte interactivo)`);
    console.log(`   - report.md (Markdown para CI/CD)`);
    console.log(`   - report.json (machine-readable)`);

    // Resumen final
    const resumen = runner.report.getResumen();
    console.log('\n📊 RESUMEN FINAL:');
    console.log(`   Total tests: ${resumen.totalTests}`);
    console.log(`   ✅ Éxitos: ${resumen.pasos}`);
    console.log(`   ❌ Fallos: ${resumen.fallos}`);
    console.log(`   ⏭️  Saltos: ${resumen.saltos}`);
    console.log(`   📈 Tasa éxito: ${resumen.tasa}`);

    if (resumen.fallos > 0) {
      console.log('\n⚠️  HAY FALLOS - Revisar reportes para detalles');
      process.exit(1);
    } else {
      console.log('\n🎉 TODAS LAS PRUEBAS PASARON');
      process.exit(0);
    }
  } catch (e) {
    console.error('\n💥 ERROR CRÍTICO:', e.message);
    console.error(e.stack);
    process.exit(1);
  }
}

main();
