/**
 * Runner principal - orquesta la ejecución de todas las suites
 */
import { join } from 'path';
import bcrypt from 'bcryptjs';
import { ClientoHTTP, ClientoRabbitMQ, ClientoPostgres } from '../utils.js';
import { Configuracion } from '../config.js';
import { Reporta } from '../reporter/reportes.js';
import { suite01Autenticacion } from '../suites/01-auth.js';
import { suite02Acceso } from '../suites/02-role-access.js';
import { suite03Catalogo } from '../suites/03-catalog.js';
import { suite04Tiendas } from '../suites/04-stores.js';
import { suite05Ordenes } from '../suites/05-orders.js';
import { suite06Carrito } from '../suites/06-cart.js';
import { suite07Logistica } from '../suites/07-logistics.js';
import { suite08Comisiones } from '../suites/08-commissions.js';
import { suite09Finanzas } from '../suites/09-finanzas.js';
import { suite10Admin } from '../suites/10-admin.js';
import { suite11Fuzzing } from '../suites/11-fuzzing.js';
import { suite12Concurrencia } from '../suites/12-concurrency.js';
import { suite13Carga } from '../suites/13-load.js';
import { suite14Queue } from '../suites/14-queue.js';
import { suite15Static } from '../suites/15-static.js';
import { suite16Owas } from '../suites/16-owasp.js';
import { suite17Esquema } from '../suites/17-schema.js';
import { suite18Field } from '../suites/18-field.js';
import { suite19Identity } from '../suites/19-identity.js';
import { suite20Multitenant } from '../suites/20-multitenant.js';

export class Runner {
  constructor(config) {
    this.config = config;
    this.client = new ClientoHTTP(config.baseUrl);
    this.rabbit = new ClientoRabbitMQ();
    this.pg = new ClientoPostgres();
    this.report = new Reporta();
    this.adminToken = null;
    this.vendedorToken = null;
    this.compradorToken = null;
    this.logisticaToken = null;
  }

  async runPreflight() {
    console.log('🔍 Ejecutando preflight...');
    const start = Date.now();
    const res = await this.client.get('/health');
    const duration = Date.now() - start;
    const pass = res.status === 200 && res.body?.api_gateway === 'ok';
    this.report.addSuiteResult('00-preflight', { pass, duration, status: res.status, body: res.body, tests: 1, pasos: pass ? 1 : 0, fallos: pass ? 0 : 1 });
    return { pass, duration };
  }

  /**
   * Garantiza el usuario del rol logistica (sistema ADR-03): el registro
   * publico solo admite vendedor/comprador, asi que se siembra via DB con el
   * mismo hash bcryptjs que usa identity-service. Auto-recuperable ante
   * borrados o hashes corruptos.
   */
  async asegurarLogistica() {
    try {
      const correo = 'logistica.qa@test.com';
      await this.pg.query(
        `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
         VALUES (gen_random_uuid(), 'QA Logistica', $1, $2, 'logistica')
         ON CONFLICT (correo) DO NOTHING`,
        [correo, bcrypt.hashSync('Logistica123!', 10)],
      );
      return true;
    } catch (e) {
      console.log(`   ⚠️  Seed logistica via DB fallo: ${e.message}`);
      return false;
    }
  }

  /** Login con reintento ante 429 (gateway: 10 /auth/login por minuto por IP). */
  async loginConReintento(correo, pass) {
    for (let i = 0; i < 4; i++) {
      const r = await this.client.post('/api/v1/auth/login', { correo, contrasena: pass });
      if (r.status === 200 || r.status === 201) return r;
      if (r.status === 429) {
        const ra = Number(r.body?.detalles?.reintente_en_seg) || 65;
        await new Promise((res) => setTimeout(res, Math.min(ra, 65) * 1000));
        continue;
      }
      return r;
    }
    return this.client.post('/api/v1/auth/login', { correo, contrasena: pass });
  }

  async loginUsers() {
    // Login admin
    const adminLogin = await this.loginConReintento('admin@core-engine.test', 'AdminCore Engine2026!');
    if (adminLogin.status === 200 || adminLogin.status === 201) {
      this.adminToken = adminLogin.body.access_token;
    }

    // Crear y login vendedor
    const regV = await this.client.post('/api/v1/auth/registro', {
      nombre: 'Vendedor QA',
      correo: 'vendedor.qa@test.com',
      contrasena: 'Contrasena123!',
      rol: 'vendedor',
    });
    if (regV.status === 200 || regV.status === 201) {
      this.vendedorToken = regV.body.access_token;
    } else {
      const loginV = await this.loginConReintento('vendedor.qa@test.com', 'Contrasena123!');
      if (loginV.status === 200 || loginV.status === 201) {
        this.vendedorToken = loginV.body.access_token;
      }
    }

    // Crear y login comprador
    const regC = await this.client.post('/api/v1/auth/registro', {
      nombre: 'Comprador QA',
      correo: 'comprador.qa@test.com',
      contrasena: 'Contrasena123!',
      rol: 'comprador',
    });
    if (regC.status === 200 || regC.status === 201) {
      this.compradorToken = regC.body.access_token;
    } else {
      const loginC = await this.loginConReintento('comprador.qa@test.com', 'Contrasena123!');
      if (loginC.status === 200 || loginC.status === 201) {
        this.compradorToken = loginC.body.access_token;
      }
    }

    // Login logistica (rol de sistema ADR-03: no auto-registrable, seed via DB)
    const loginL = await this.loginConReintento('logistica.qa@test.com', 'Logistica123!');
    if (loginL.status === 200 || loginL.status === 201) {
      this.logisticaToken = loginL.body.access_token;
    }
  }

  async runAll() {
    console.log('🚀 Iniciando QA Harness completo...');
    const startTime = Date.now();

    // Soporte --suites: si se especifica, solo corre esas (por nombre exacto o
    // prefijo, p.ej. '17' ejecuta '17-schema'). Vacío = todas.
    const solo = (this.config.suites || []).map((s) => s.trim()).filter(Boolean);
    const activo = (clave) => solo.length === 0 || solo.some((s) => clave === s || clave.startsWith(`${s}-`) || `${s}`.startsWith(`${clave}-`));
    const noAuth = new Set(['00-preflight', '17-schema']);
    const necesitaAuth = solo.length === 0 || solo.some((s) => !noAuth.has(s));
    const ventana = (ms) => new Promise((r) => setTimeout(r, ms));

    // Preflight
    await this.runPreflight();

    // Schema preflight (no requiere auth)
    if (activo('17-schema')) {
      await this.runSuite('17-schema', () => suite17Esquema(this.pg, this.config));
    }

    // Field service (backend móvil) — requiere tenant propio + coordinador sembrado
    if (activo('18-field')) {
      await this.runSuite('18-field', () => suite18Field(this.client, this.config, this.adminToken, this.compradorToken, this.logisticaToken, this.pg));
    }

    // Identity / alta administrativa de usuarios (sustituto de Firebase Auth)
    if (activo('19-identity')) {
      await this.runSuite('19-identity', () => suite19Identity({ client: this.client, pg: this.pg, assert: this.assert, T: this.T, config: this.config }));
    }

    // Multitenant: aislamiento por tenant_id en field-service
    if (activo('20-multitenant')) {
      await this.runSuite('20-multitenant', () => suite20Multitenant({ client: this.client, pg: this.pg, config: this.config }));
    }

    // Login users first (solo si alguna suite lo requiere)
    if (necesitaAuth) {
      console.log('\n🔐 Autenticando usuarios...');
      // Las suites previas (p.ej. 19-identity) pueden haber consumido la cuota
      // de /auth/login (10/min por IP). Abrimos ventana para resetear el contador.
      await ventana(65000);
      await this.asegurarLogistica();
      await this.loginUsers();
      console.log(`   Admin: ${this.adminToken ? '✅' : '❌'}`);
      console.log(`   Vendedor: ${this.vendedorToken ? '✅' : '❌'}`);
      console.log(`   Comprador: ${this.compradorToken ? '✅' : '❌'}`);
      console.log(`   Logistica: ${this.logisticaToken ? '✅' : '❌'}`);

      // El gateway limita /auth/login ~10-12/min por IP: abrir ventana antes de
      // las suites que dependen de logins frescos (01 hace ~6, 02 hasta 12).
      if (activo('01-auth')) {
        await ventana(65000);
        console.log('⏳ Ventana de rate-limit abierta (65s)');
      }

      // Suites de autenticación y seguridad
      if (activo('01-auth')) {
        await this.runSuite('01-auth', () => suite01Autenticacion(this.client, this.config, this.adminToken));
      }
      if (activo('01-auth') || activo('02-role-access')) {
        await ventana(65000);
        console.log('⏳ Ventana de rate-limit abierta (65s)');
      }
      if (activo('02-role-access')) {
        await this.runSuite('02-role-access', () => suite02Acceso(this.client, this.config, this.adminToken, this.compradorToken, this.logisticaToken));
      }

      // Suites de negocio (necesitan tokens)
      if (activo('03-catalog')) await this.runSuite('03-catalog', () => suite03Catalogo(this.client, this.config, this.adminToken));
      if (activo('04-stores')) await this.runSuite('04-stores', () => suite04Tiendas(this.client, this.config, this.vendedorToken, this.adminToken));
      if (activo('05-orders')) await this.runSuite('05-orders', () => suite05Ordenes(this.client, this.config, this.compradorToken, this.adminToken, this.vendedorToken));
      if (activo('06-cart')) await this.runSuite('06-cart', () => suite06Carrito(this.client, this.config, this.compradorToken, this.adminToken, this.vendedorToken));
      if (activo('07-logistics')) await this.runSuite('07-logistics', () => suite07Logistica(this.client, this.config, this.adminToken, this.compradorToken, this.vendedorToken, this.logisticaToken));
      if (activo('08-commissions')) await this.runSuite('08-commissions', () => suite08Comisiones(this.client, this.config, this.vendedorToken, this.adminToken, this.compradorToken));
      if (activo('09-finanzas')) await this.runSuite('09-finanzas', () => suite09Finanzas(this.client, this.config, this.adminToken));
      if (activo('10-admin')) await this.runSuite('10-admin', () => suite10Admin(this.client, this.config, this.adminToken));
    }

    // Suites de testing agresivo (auth opcional por suite)
    if (activo('11-fuzzing')) await this.runSuite('11-fuzzing', () => suite11Fuzzing(this.client, this.config, this.adminToken, this.vendedorToken));
    if (activo('12-concurrency')) await this.runSuite('12-concurrency', () => suite12Concurrencia(this.client, this.config, this.compradorToken, this.adminToken, this.vendedorToken));
    if (activo('14-queue')) await this.runSuite('14-queue', () => suite14Queue(this.client, this.rabbit, this.pg, this.config));
    if (activo('16-owasp')) await this.runSuite('16-owasp', () => suite16Owas(this.client, this.config, this.adminToken, this.vendedorToken, this.compradorToken));
    if (activo('13-load')) await this.runSuite('13-load', () => suite13Carga(this.client, this.config));
    if (activo('15-static')) await this.runSuite('15-static', () => suite15Static(this.client, this.config));

    const totalDuration = Date.now() - startTime;
    console.log(`\n✅ QA Harness completado en ${totalDuration}ms`);
    console.log(`📊 Resumen: ${JSON.stringify(this.report.getResumen())}`);
  }

  async runSuite(name, fn) {
    console.log(`\n▶️  Suite: ${name}`);
    const start = Date.now();
    try {
      const result = await fn();
      const duration = Date.now() - start;
      const passedTests = result.resultados ? result.resultados.filter(r => r.pass).length : 0;
      const totalTests = result.resultados ? result.resultados.length : 0;
      const pass = passedTests > 0;
      this.report.addSuiteResult(name, { 
        ...result, 
        duration, 
        tests: totalTests, 
        pasos: passedTests, 
        fallos: totalTests - passedTests 
      });
      console.log(`   ${pass ? '✅' : '❌'} ${name} (${duration}ms) - ${passedTests}/${totalTests} tests passed`);
    } catch (e) {
      const duration = Date.now() - start;
      console.log(`   ❌ ${name} - ERROR: ${e.message}`);
      this.report.addSuiteResult(name, { pass: false, error: e.message, duration, tests: 0, pasos: 0, fallos: 1 });
    }
  }

  generateReports() {
    return {
      html: this.report.generarHTML(),
      markdown: this.report.generarMarkdown(),
      json: this.report.generarJSON(),
    };
  }

  saveReports(outputDir) {
    const reports = this.generateReports();
    import('fs').then(fs => {
      if (!fs.existsSync(outputDir)) fs.mkdirSync(outputDir, { recursive: true });
      fs.writeFileSync(join(outputDir, 'report.html'), reports.html);
      fs.writeFileSync(join(outputDir, 'report.md'), reports.markdown);
      fs.writeFileSync(join(outputDir, 'report.json'), reports.json);
      console.log(`\n📄 Reportes guardados en: ${outputDir}`);
    });
  }
}