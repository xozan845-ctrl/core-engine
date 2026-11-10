/**
 * E2E multi-vertical (G-6): belleza y flota contra el stack real.
 *
 * Requiere el ecosistema levantado. Uso:
 *   BASE_URL=http://localhost:8080 npm run demo:verticals
 */

const BASE = process.env.BASE_URL ?? 'http://localhost:8080';
const REQUEST_TIMEOUT_MS = Number(process.env.SMOKE_REQUEST_TIMEOUT_MS ?? 10_000);

let pasos = 0;
let fallidas = 0;

function paso(nombre) {
  pasos += 1;
  process.stdout.write(`\n[${String(pasos).padStart(2, '0')}] ${nombre} ... `);
}

function ok(detalle = '') {
  console.log(`OK ${detalle}`);
}

function fail(detalle = '') {
  fallidas += 1;
  console.log(`FAIL ${detalle}`);
}

function desenvolver(json) {
  if (json && typeof json === 'object' && 'data' in json && 'meta' in json) return json.data;
  return json;
}

function sufijo() {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

async function api(metodo, ruta, cuerpo, token) {
  const res = await fetch(`${BASE}${ruta}`, {
    method: metodo,
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: cuerpo ? JSON.stringify(cuerpo) : undefined,
    signal: globalThis.AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  const texto = await res.text();
  let json;
  try {
    json = texto ? JSON.parse(texto) : null;
  } catch {
    json = texto;
  }
  return { status: res.status, json: desenvolver(json) };
}

async function crearEmpresario(suf, nombre, correo, contrasena) {
  const creado = await api('POST', '/api/v2/auth/registro', {
    nombre,
    correo,
    contrasena,
    rol: 'empresario',
  });
  if (creado.status !== 201 || !creado.json?.access_token) {
    throw new Error(`registro ${correo} → ${creado.status} ${JSON.stringify(creado.json)}`);
  }
  return creado.json.access_token;
}

async function crearEmpresa(nombre, vertical, pack, slug, token) {
  const creada = await api('POST', '/api/v2/empresas', {
    nombre,
    slug,
    vertical_id: vertical,
    pack_codigo: pack,
  }, token);
  if (creada.status !== 201) {
    throw new Error(`crear empresa ${vertical} → ${creada.status} ${JSON.stringify(creada.json)}`);
  }
  return creada.json;
}

async function main() {
  paso('Readiness del ecosistema');
  const ready = await api('GET', '/ready');
  if (ready.status !== 200 || ready.json?.status !== 'ok') {
    fail(`readiness=${ready.status}`);
    process.exit(1);
  }
  ok();

  const suf = sufijo();
  const contrasena = 'Contrasena123!';

  // ── Belleza ────────────────────────────────────────────────────────────
  paso('Belleza: alta de empresario y tenant desde pack_belleza');
  const correoBelleza = `belleza.${suf}@core-engine.test`;
  let tokenBelleza = await crearEmpresario(suf, 'Empresaria Belleza', correoBelleza, contrasena);
  const empresaBelleza = await crearEmpresa(
    `Salon E2E ${suf}`,
    'belleza',
    'pack_belleza',
    `salon-e2e-${suf}`,
    tokenBelleza,
  );
  tokenBelleza = (await api('POST', '/api/v2/auth/login', {
    correo: correoBelleza,
    contrasena,
  })).json.access_token;
  ok(`empresa=${empresaBelleza.id} v=${empresaBelleza.pack_version}`);

  paso('Belleza: parte, recurso, disponibilidad y cita');
  const parte = await api('POST', `/api/v2/empresas/${empresaBelleza.id}/partes`, {
    tipo: 'persona',
    nombre: `Cliente E2E ${suf}`,
  }, tokenBelleza);
  if (parte.status !== 201) throw new Error(`parte → ${parte.status} ${JSON.stringify(parte.json)}`);

  const recurso = await api('POST', '/api/v2/scheduling/recursos', {
    tipo: 'profesional',
    nombre: `Estilista E2E ${suf}`,
    atributos: { especialidad: 'corte' },
  }, tokenBelleza);
  if (recurso.status !== 201) throw new Error(`recurso → ${recurso.status} ${JSON.stringify(recurso.json)}`);

  const disponibilidad = await api('POST', '/api/v2/scheduling/disponibilidad', {
    recurso_id: recurso.json.id,
    dia_semana: 1,
    inicia_hora: '08:00',
    termina_hora: '18:00',
    zona_horaria: 'America/Managua',
  }, tokenBelleza);
  if (disponibilidad.status !== 201) {
    throw new Error(`disponibilidad → ${disponibilidad.status} ${JSON.stringify(disponibilidad.json)}`);
  }

  const cita = await api('POST', '/api/v2/scheduling/citas', {
    recurso_id: recurso.json.id,
    parte_id: parte.json.id,
    inicia_en: '2032-04-12T14:00:00Z',
    termina_en: '2032-04-12T15:00:00Z',
    notas: 'Cita E2E',
  }, tokenBelleza);
  if (cita.status !== 201 || cita.json?.estado !== 'reservada') {
    throw new Error(`cita → ${cita.status} ${JSON.stringify(cita.json)}`);
  }
  ok(`cita=${cita.json.id}`);

  // ── Flota ──────────────────────────────────────────────────────────────
  paso('Flota: alta de empresario y tenant desde pack_flota');
  const correoFlota = `flota.${suf}@core-engine.test`;
  let tokenFlota = await crearEmpresario(suf, 'Empresario Flota', correoFlota, contrasena);
  const empresaFlota = await crearEmpresa(
    `Flota E2E ${suf}`,
    'flota',
    'pack_flota',
    `flota-e2e-${suf}`,
    tokenFlota,
  );
  tokenFlota = (await api('POST', '/api/v2/auth/login', {
    correo: correoFlota,
    contrasena,
  })).json.access_token;
  ok(`empresa=${empresaFlota.id} v=${empresaFlota.pack_version}`);

  paso('Flota: vehículo, ruta y parada');
  const vehiculo = await api('POST', '/api/v2/field/vehiculos', {
    placa: `E2E-${suf.slice(-6).toUpperCase()}`,
    tipo: 'camioneta',
    marca: 'Toyota',
    modelo: 'Hilux',
    anio: 2026,
  }, tokenFlota);
  if (vehiculo.status !== 201) throw new Error(`vehiculo → ${vehiculo.status} ${JSON.stringify(vehiculo.json)}`);

  const ruta = await api('POST', '/api/v2/field/rutas', {
    nombre: `Ruta E2E ${suf}`,
    descripcion: 'Reparto E2E',
    vehiculoAsignadoId: vehiculo.json.id,
  }, tokenFlota);
  if (ruta.status !== 201) throw new Error(`ruta → ${ruta.status} ${JSON.stringify(ruta.json)}`);

  const parada = await api('POST', `/api/v2/field/rutas/${ruta.json.id}/paradas`, {
    nombre: `Parada E2E ${suf}`,
    direccion: 'Managua',
    lat: 12.1364,
    lng: -86.2514,
    orden: 1,
  }, tokenFlota);
  if (parada.status !== 201) throw new Error(`parada → ${parada.status} ${JSON.stringify(parada.json)}`);
  ok(`ruta=${ruta.json.id}`);

  console.log(`\n${pasos - fallidas}/${pasos} pasos OK`);
  if (fallidas > 0) process.exit(1);
}

main().catch((error) => {
  console.error(`\nFAIL ${error.message}`);
  process.exit(1);
});
