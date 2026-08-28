/**
 * Suite 20 · Multitenant (aislamiento por tenant_id en field-service)
 *
 * Valida que los datos de un tenant NO son visibles por otro tenant:
 *  - dos coordinadores de tenants distintos (A y B)
 *  - recursos creados por A no aparecen en los listados ni lecturas de B
 *  - lectura directa por id de B a un recurso de A → 404
 *  - recursos de B no aparecen en A
 */
import bcrypt from 'bcryptjs';
import { ClientoHTTP, ClientoPostgres } from '../utils.js';

function assert(res, cond, detail) {
  return { status: cond ? res.status : `${res.status}❌`, pass: !!cond, detail };
}

async function sembrar(pg, correo, rol, tenantId, pass = 'Multi1234!') {
  const hash = bcrypt.hashSync(pass, 10);
  await pg.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol, tenant_id, creado_en)
     VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, NOW())
     ON CONFLICT (correo) DO UPDATE SET tenant_id = $5, rol = $4, contrasena_hash = $3`,
    [correo, correo, hash, rol, tenantId],
  );
}

export async function suite20Multitenant({ client, pg, config }) {
  const errores = [];
  const resultados = [];
  const T = (test, res, detail = '') => {
    resultados.push({ test, status: res.status, pass: res.pass });
    if (!res.pass) errores.push(`${test} :: ${detail || res.status}`);
  };
  const FIELD = '/api/v1/field';
  const limpieza = [];

  try {
    const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
    const tenantA = (await pg.query('SELECT gen_random_uuid() AS id'))[0].id;
    const tenantB = (await pg.query('SELECT gen_random_uuid() AS id'))[0].id;
    const correoA = `tenantA.${suf}@test.com`;
    const correoB = `tenantB.${suf}@test.com`;
    await sembrar(pg, correoA, 'coordinador', tenantA);
    await sembrar(pg, correoB, 'coordinador', tenantB);
    const loginA = await client.post('/api/v1/auth/login', { correo: correoA, contrasena: 'Multi1234!' });
    const loginB = await client.post('/api/v1/auth/login', { correo: correoB, contrasena: 'Multi1234!' });
    const tokenA = loginA.body?.access_token;
    const tokenB = loginB.body?.access_token;
    const authA = { authorization: `Bearer ${tokenA}` };
    const authB = { authorization: `Bearer ${tokenB}` };
    T('multitenant: ambos coordinadores loguean', assert(loginA, !!tokenA && !!tokenB), JSON.stringify([loginA.status, loginB.status]));

    // ── Clientes: aislamiento ────────────────────────────────────────────────
    const cA = await client.post(`${FIELD}/clientes`, { nombreCompleto: `Cli A ${suf}` }, authA);
    T('clientes: A crea', assert(cA, [200, 201].includes(cA.status) && !!cA.body?.id), JSON.stringify(cA.status));
    const idA = cA.body?.id;
    if (idA) limpieza.push(['clientes', idA, authA]);
    const cB = await client.post(`${FIELD}/clientes`, { nombreCompleto: `Cli B ${suf}` }, authB);
    const idB = cB.body?.id;
    if (idB) limpieza.push(['clientes', idB, authB]);

    const listA = await client.get(`${FIELD}/clientes`, authA);
    T('clientes: A lista incluye suya', assert(listA, Array.isArray(listA.body) && listA.body.some((x) => x.id === idA)), JSON.stringify(listA.status));
    const listB = await client.get(`${FIELD}/clientes`, authB);
    T('clientes: B lista NO incluye de A (aislamiento)', assert(listB, Array.isArray(listB.body) && !listB.body.some((x) => x.id === idA)), JSON.stringify(listB.status));
    T('clientes: A lista NO incluye de B (aislamiento)', assert(listA, !listA.body.some((x) => x.id === idB)), JSON.stringify(listA.status));

    const getPorB = await client.get(`${FIELD}/clientes/${idA}`, authB);
    T('clientes: B lee recurso de A → 404 (aislamiento fila)', assert(getPorB, getPorB.status === 404), JSON.stringify(getPorB.status));
    const getPorA = await client.get(`${FIELD}/clientes/${idB}`, authA);
    T('clientes: A lee recurso de B → 404 (aislamiento fila)', assert(getPorA, getPorA.status === 404), JSON.stringify(getPorA.status));

    // ── Personal: aislamiento ────────────────────────────────────────────────
    const pA = await client.post(`${FIELD}/personal`, { nombre: `Pers A ${suf}`, apellido: 'Ais' }, authA);
    const pidA = pA.body?.id;
    if (pidA) limpieza.push(['personal', pidA, authA]);
    const pListB = await client.get(`${FIELD}/personal`, authB);
    T('personal: B lista NO incluye de A (aislamiento)', assert(pListB, Array.isArray(pListB.body) && !pListB.body.some((x) => x.id === pidA)), JSON.stringify(pListB.status));

    // ── Sin token: field exige tenant ────────────────────────────────────────
    const sinTok = await client.get(`${FIELD}/clientes`, {});
    T('field: sin token → 401', assert(sinTok, sinTok.status === 401), JSON.stringify(sinTok.status));
  } catch (e) {
    errores.push(`Suite 20: ${e.message}`);
  } finally {
    for (const [tipo, id, auth] of limpieza) {
      await client.delete(`${FIELD}/${tipo}/${id}`, auth).catch(() => {});
    }
  }

  const pasados = resultados.filter((r) => r.pass).length;
  return {
    test: 'Multitenant · aislamiento por tenant (field-service)',
    pass: errores.length === 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}
