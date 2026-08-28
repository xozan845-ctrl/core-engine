/**
 * Suite 18 — Field Service (backend móvil / app-test)
 *
 * Cobertura exhaustiva del dominio de logística de campo:
 * personal, clientes, vehículos, rutas+paradas, pedidos, asistencia,
 * incidencias, tracking GPS, visitas, cumplimiento y el endpoint de
 * sincronización offline (SyncDto). Valida CRUD, reglas de rol y 404/401/403.
 *
 * Nota: field-service exige tenant_id no nulo en el contexto (x-tenant).
 * El admin sembrado y logistica.qa tienen tenant_id NULL, así que esta
 * suite siembra un usuario coordinador con un tenant propio y opera con él.
 */
import bcrypt from 'bcryptjs';
import { ClientoHTTP, ClientoPostgres } from '../utils.js';

const FIELD = '/api/v1/field';

async function sembrarUsuario(pg, correo, rol, tenantId, password = 'Field1234!') {
  const hash = bcrypt.hashSync(password, 10);
  await pg.query(
    `INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol, tenant_id, creado_en)
     VALUES (gen_random_uuid(), 'QA Field', $1, $2, $3, $4, NOW())
     ON CONFLICT (correo) DO UPDATE SET tenant_id = $4, rol = $3, contrasena_hash = $2`,
    [correo, hash, rol, tenantId],
  );
}

function assert(res, cond, detail) {
  return { status: cond ? res.status : `${res.status}❌`, pass: !!cond, detail };
}

export async function suite18Field(client, cfg, adminToken, compradorToken, logisticaToken, pg) {
  const errores = [];
  const resultados = [];
  const T = (test, res, detail = '') => {
    resultados.push({ test, status: res.status, pass: res.pass });
    if (!res.pass) errores.push(`${test} :: ${detail || res.status}`);
  };

  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  const tenantId = (await pg.query('SELECT gen_random_uuid() AS id'))[0].id;
  const correo = `field.qa.${suf}@test.com`;

  // Sembrar coordinador (rol con acceso total a field, incluídos deletes)
  await sembrarUsuario(pg, correo, 'coordinador', tenantId);
  const login = await client.post('/api/v1/auth/login', { correo, contrasena: 'Field1234!' });
  if (login.status !== 200 && login.status !== 201) {
    errores.push(`No se pudo autenticar usuario field: ${JSON.stringify(login.body)}`);
    return { test: 'Field Service (backend móvil)', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }
  const token = login.body.access_token;
  const auth = { authorization: `Bearer ${token}` };

  // Usuario comprador propio (fresco, evita token nulo por rate-limit del runner)
  const compCorreo = `comprador.qa.${suf}@test.com`;
  const regC = await client.post('/api/v1/auth/registro', { nombre: 'Comprador QA', correo: compCorreo, contrasena: 'Contrasena123!', rol: 'comprador' });
  let compToken = regC.body?.access_token;
  if (!compToken) {
    const lc = await client.post('/api/v1/auth/login', { correo: compCorreo, contrasena: 'Contrasena123!' });
    compToken = lc.body?.access_token;
  }
  const authC = { authorization: `Bearer ${compToken}` };

  const limpiar = async () => {
    try {
      await pg.query('DELETE FROM field.asistencia WHERE tenant_id = $1', [tenantId]);
      await pg.query('DELETE FROM field.tracking WHERE tenant_id = $1', [tenantId]);
      await pg.query('DELETE FROM field.visitas WHERE tenant_id = $1', [tenantId]);
      await pg.query('DELETE FROM field.cumplimiento WHERE tenant_id = $1', [tenantId]);
      await pg.query('DELETE FROM field.paradas WHERE tenant_id = $1', [tenantId]);
      await pg.query('DELETE FROM identity.usuarios WHERE correo = $1', [correo]);
    } catch (_) { /* best-effort */ }
  };

  try {
    // ── PERSONAL ──────────────────────────────────────────────────────────
    const pCrea = await client.post(`${FIELD}/personal`, { nombre: 'Juan', apellido: 'Perez', cargo: 'conductor', telefono: '123' }, auth);
    T('Personal: alta (TC-01)', assert(pCrea, [200, 201].includes(pCrea.status), JSON.stringify(pCrea.body)), JSON.stringify(pCrea.body));
    const pId = pCrea.body?.id;
    const pGet = await client.get(`${FIELD}/personal/${pId}`, auth);
    T('Personal: obtener por id', assert(pGet, pGet.status === 200 && pGet.body?.id === pId), JSON.stringify(pGet.body));
    const pList = await client.get(`${FIELD}/personal`, auth);
    T('Personal: listar contiene creado', assert(pList, Array.isArray(pList.body) && pList.body.some((x) => x.id === pId)), '');
    const pUpd = await client.patch(`${FIELD}/personal/${pId}`, { nombre: 'Juan Mod' }, auth);
    T('Personal: actualizar', assert(pUpd, pUpd.status === 200 && pUpd.body?.nombre === 'Juan Mod'), JSON.stringify(pUpd.body));
    const pDel = await client.delete(`${FIELD}/personal/${pId}`, auth);
    T('Personal: eliminar', assert(pDel, [200, 204].includes(pDel.status)), JSON.stringify(pDel.status));
    const p404 = await client.get(`${FIELD}/personal/${pId}`, auth);
    T('Personal: obtener eliminado → 404', assert(p404, p404.status === 404), JSON.stringify(p404.status));
    const pNoTok = await client.post(`${FIELD}/personal`, { nombre: 'x' }, {});
    T('Personal: sin token → 401', assert(pNoTok, pNoTok.status === 401), JSON.stringify(pNoTok.status));
    const pForbid = await client.post(`${FIELD}/personal`, { nombre: 'x' }, authC);
    T('Personal: rol comprador → 403', assert(pForbid, pForbid.status === 403), JSON.stringify(pForbid.status));

    // ── CLIENTES ───────────────────────────────────────────────────────────
    const cCrea = await client.post(`${FIELD}/clientes`, { nombreCompleto: 'Cliente QA', tipoCliente: 'mayorista', telefono: '999' }, auth);
    T('Cliente: alta', assert(cCrea, [200, 201].includes(cCrea.status), JSON.stringify(cCrea.body)), JSON.stringify(cCrea.body));
    const cId = cCrea.body?.id;
    const cGet = await client.get(`${FIELD}/clientes/${cId}`, auth);
    T('Cliente: obtener', assert(cGet, cGet.status === 200 && cGet.body?.id === cId), '');
    const cUpd = await client.patch(`${FIELD}/clientes/${cId}`, { telefono: '888' }, auth);
    T('Cliente: actualizar', assert(cUpd, cUpd.status === 200 && cUpd.body?.telefono === '888'), '');
    const cDel = await client.delete(`${FIELD}/clientes/${cId}`, auth);
    T('Cliente: eliminar', assert(cDel, [200, 204].includes(cDel.status)), '');
    const cForbid = await client.post(`${FIELD}/clientes`, { nombreCompleto: 'x' }, authC);
    T('Cliente: rol comprador → 403', assert(cForbid, cForbid.status === 403), '');

    // ── VEHICULOS ─────────────────────────────────────────────────────────
    const vCrea = await client.post(`${FIELD}/vehiculos`, { placa: `QA-${suf.toUpperCase()}`, tipo: 'camioneta', estado: 'disponible' }, auth);
    T('Vehículo: alta', assert(vCrea, [200, 201].includes(vCrea.status), JSON.stringify(vCrea.body)), JSON.stringify(vCrea.body));
    const vId = vCrea.body?.id;
    const vGet = await client.get(`${FIELD}/vehiculos/${vId}`, auth);
    T('Vehículo: obtener', assert(vGet, vGet.status === 200 && vGet.body?.id === vId), '');
    const vUpd = await client.patch(`${FIELD}/vehiculos/${vId}`, { estado: 'en_ruta' }, auth);
    T('Vehículo: actualizar', assert(vUpd, vUpd.status === 200 && vUpd.body?.estado === 'en_ruta'), '');
    const vDup = await client.post(`${FIELD}/vehiculos`, { placa: `QA-${suf.toUpperCase()}` }, auth);
    // Nota: el esquema no impone UNIQUE en placa; se acepta 201 (sin unique) o 409 (si se añade).
    T('Vehículo: placa duplicada (idempotente o 409)', assert(vDup, [200, 201, 409].includes(vDup.status), JSON.stringify(vDup.status)), JSON.stringify(vDup.status));
    const vDel = await client.delete(`${FIELD}/vehiculos/${vId}`, auth);
    T('Vehículo: eliminar', assert(vDel, [200, 204].includes(vDel.status)), '');
    const vForbid = await client.post(`${FIELD}/vehiculos`, { placa: 'X' }, authC);
    T('Vehículo: rol comprador → 403', assert(vForbid, vForbid.status === 403), '');

    // ── RUTAS + PARADAS ───────────────────────────────────────────────────
    const rCrea = await client.post(`${FIELD}/rutas`, { nombre: 'Ruta QA', descripcion: 'ruta test' }, auth);
    T('Ruta: alta', assert(rCrea, [200, 201].includes(rCrea.status), JSON.stringify(rCrea.body)), JSON.stringify(rCrea.body));
    const rId = rCrea.body?.id;
    const rGet0 = await client.get(`${FIELD}/rutas/${rId}`, auth);
    T('Ruta: obtener (con paradas [])', assert(rGet0, rGet0.status === 200 && Array.isArray(rGet0.body?.paradas)), '');
    const pCrea2 = await client.post(`${FIELD}/rutas/${rId}/paradas`, { nombre: 'Parada 1', orden: 1, lat: 1.1, lng: 2.2, tipo: 'cliente' }, auth);
    T('Parada: agregar a ruta', assert(pCrea2, [200, 201].includes(pCrea2.status), JSON.stringify(pCrea2.body)), JSON.stringify(pCrea2.body));
    const pId2 = pCrea2.body?.id;
    const rGet1 = await client.get(`${FIELD}/rutas/${rId}`, auth);
    T('Ruta: obtener incluye parada', assert(rGet1, rGet1.status === 200 && rGet1.body?.paradas?.some((x) => x.id === pId2)), '');
    const pUpd2 = await client.patch(`${FIELD}/rutas/${rId}/paradas/${pId2}`, { nombre: 'Parada 1 Mod' }, auth);
    T('Parada: actualizar', assert(pUpd2, pUpd2.status === 200 && pUpd2.body?.nombre === 'Parada 1 Mod'), '');
    const fakeParada = (await pg.query('SELECT gen_random_uuid() AS id'))[0].id;
    const pBadRuta = await client.patch(`${FIELD}/rutas/${rId}/paradas/${fakeParada}`, { nombre: 'x' }, auth);
    T('Parada: id inexistente → 404', assert(pBadRuta, pBadRuta.status === 404), JSON.stringify(pBadRuta.status));
    const asig = await client.post(`${FIELD}/rutas/${rId}/asignar`, { personalIds: [], vehiculoId: vId && vId }, auth);
    T('Ruta: asignar', assert(asig, [200, 201, 204].includes(asig.status)), JSON.stringify(asig.status));
    const rList = await client.get(`${FIELD}/rutas`, auth);
    T('Ruta: listar contiene creada', assert(rList, Array.isArray(rList.body) && rList.body.some((x) => x.id === rId)), '');
    const rDel = await client.delete(`${FIELD}/rutas/${rId}`, auth);
    T('Ruta: eliminar (cascada paradas)', assert(rDel, [200, 204].includes(rDel.status)), '');
    const rForbid = await client.post(`${FIELD}/rutas`, { nombre: 'x' }, authC);
    T('Ruta: rol comprador → 403', assert(rForbid, rForbid.status === 403), '');

    // ── PEDIDOS ────────────────────────────────────────────────────────────
    const eCrea = await client.post(`${FIELD}/pedidos`, { cliente: 'Cli', direccionEntrega: 'Calle 1', lat: 1.0, lng: 2.0 }, auth);
    T('Pedido: alta', assert(eCrea, [200, 201].includes(eCrea.status), JSON.stringify(eCrea.body)), JSON.stringify(eCrea.body));
    const eId = eCrea.body?.id;
    const eGet = await client.get(`${FIELD}/pedidos/${eId}`, auth);
    T('Pedido: obtener', assert(eGet, eGet.status === 200 && eGet.body?.id === eId), '');
    const eEst = await client.patch(`${FIELD}/pedidos/${eId}/estado`, { estado: 'en_camino', motivo: 'salio' }, auth);
    T('Pedido: cambiar estado', assert(eEst, eEst.status === 200 && eEst.body?.estado === 'en_camino'), JSON.stringify(eEst.body));
    const eList = await client.get(`${FIELD}/pedidos`, auth);
    T('Pedido: listar contiene', assert(eList, Array.isArray(eList.body) && eList.body.some((x) => x.id === eId)), '');
    const eDel = await client.delete(`${FIELD}/pedidos/${eId}`, auth);
    T('Pedido: eliminar', assert(eDel, [200, 204].includes(eDel.status)), '');
    const eForbid = await client.post(`${FIELD}/pedidos`, { cliente: 'x' }, authC);
    T('Pedido: rol comprador → 403', assert(eForbid, eForbid.status === 403), '');

    // ── ASISTENCIA ─────────────────────────────────────────────────────────
    const aCreaP = await client.post(`${FIELD}/personal`, { nombre: 'Aux Asist' }, auth);
    const aPid = aCreaP.body?.id;
    const aCrea = await client.post(`${FIELD}/asistencia`, { personalId: aPid, tipo: 'entrada', estadoPuntualidad: 'puntual', lat: 1, lng: 2 }, auth);
    T('Asistencia: registrar', assert(aCrea, [200, 201].includes(aCrea.status), JSON.stringify(aCrea.body)), JSON.stringify(aCrea.body));
    const aList = await client.get(`${FIELD}/asistencia?personalId=${aPid}`, auth);
    T('Asistencia: listar por personal', assert(aList, Array.isArray(aList.body) && aList.body.length >= 1), '');
    const aForbid = await client.post(`${FIELD}/asistencia`, { personalId: aPid, tipo: 'entrada', estadoPuntualidad: 'puntual' }, authC);
    T('Asistencia: rol comprador → 403', assert(aForbid, aForbid.status === 403), '');

    // ── INCIDENCIAS ────────────────────────────────────────────────────────
    const iCrea = await client.post(`${FIELD}/incidencias`, { tipo: 'mecanica', descripcion: 'pinchazo' }, auth);
    T('Incidencia: alta', assert(iCrea, [200, 201].includes(iCrea.status), JSON.stringify(iCrea.body)), JSON.stringify(iCrea.body));
    const iId = iCrea.body?.id;
    const iUpd = await client.patch(`${FIELD}/incidencias/${iId}`, { estado: 'en_progreso' }, auth);
    T('Incidencia: actualizar', assert(iUpd, iUpd.status === 200 && iUpd.body?.estado === 'en_progreso'), '');
    const iGet = await client.get(`${FIELD}/incidencias/${iId}`, auth);
    T('Incidencia: obtener', assert(iGet, iGet.status === 200 && iGet.body?.id === iId), '');
    const iDel = await client.delete(`${FIELD}/incidencias/${iId}`, auth);
    T('Incidencia: eliminar', assert(iDel, [200, 204].includes(iDel.status)), '');
    const iForbid = await client.post(`${FIELD}/incidencias`, { tipo: 'clima', descripcion: 'x' }, authC);
    T('Incidencia: rol comprador → 403', assert(iForbid, iForbid.status === 403), '');

    // ── TRACKING (GPS) ─────────────────────────────────────────────────────
    const tCrea = await client.post(`${FIELD}/tracking`, {
      registros: [
        { personalId: aPid, latitud: 1.1, longitud: 2.2, precision: 5, velocidad: 10, rumbo: 90 },
        { personalId: aPid, latitud: 1.2, longitud: 2.3, precision: 5 },
      ],
    }, auth);
    T('Tracking: registro bulk', assert(tCrea, [200, 201].includes(tCrea.status) && Array.isArray(tCrea.body) && tCrea.body.length === 2), JSON.stringify(tCrea.status));
    const tList = await client.get(`${FIELD}/tracking?personalId=${aPid}`, auth);
    T('Tracking: listar por personal', assert(tList, Array.isArray(tList.body) && tList.body.length >= 2), '');

    // ── VISITAS (telemetría) ───────────────────────────────────────────────
    const vCrea2 = await client.post(`${FIELD}/visitas`, { personalId: aPid, tipoActividad: 'entrega', resultado: 'visitado', lat: 1, lng: 2 }, auth);
    T('Visita: registrar', assert(vCrea2, [200, 201].includes(vCrea2.status), JSON.stringify(vCrea2.body)), JSON.stringify(vCrea2.body));
    const vList = await client.get(`${FIELD}/visitas?personalId=${aPid}`, auth);
    T('Visita: listar', assert(vList, Array.isArray(vList.body) && vList.body.length >= 1), '');

    // ── CUMPLIMIENTO ───────────────────────────────────────────────────────
    const rCrea2 = await client.post(`${FIELD}/rutas`, { nombre: 'Ruta Cumplimiento' }, auth);
    const rId2 = rCrea2.body?.id;
    const cG = await client.post(`${FIELD}/cumplimiento`, { rutaId: rId2, fecha: new Date().toISOString().slice(0, 10), metricas: { entregas: 5, fallidas: 1 } }, auth);
    T('Cumplimiento: guardar', assert(cG, [200, 201].includes(cG.status), JSON.stringify(cG.body)), JSON.stringify(cG.body));
    const cO = await client.get(`${FIELD}/cumplimiento/${rId2}/${new Date().toISOString().slice(0, 10)}`, auth);
    T('Cumplimiento: obtener', assert(cO, cO.status === 200 && cO.body?.metricas != null), JSON.stringify(cO.body));

    // ── SYNC OFFLINE (app-test) ─────────────────────────────────────────────
    const syncId = (await pg.query('SELECT gen_random_uuid() AS id'))[0].id;
    const syncParadaId = (await pg.query('SELECT gen_random_uuid() AS id'))[0].id;
    const sync = await client.post(`${FIELD}/sync`, {
      operaciones: [
        { tipo: 'personal', payload: { id: syncId, nombre: 'Sync Persona', cargo: 'auxiliar' } },
        { tipo: 'parada', payload: { id: syncParadaId, rutaId: rId2, nombre: 'Sync Parada', orden: 3, lat: 5, lng: 6 } },
      ],
    }, auth);
    T('Sync: respuesta 200 con resultados', assert(sync, [200, 201].includes(sync.status) && Array.isArray(sync.body), JSON.stringify(sync.status)), JSON.stringify(sync.body));
    const syncOk = Array.isArray(sync.body) && sync.body.length === 2 && sync.body.every((r) => r.ok === true);
    T('Sync: ambas operaciones ok', assert(sync, syncOk, JSON.stringify(sync.body)), JSON.stringify(sync.body));
    const sList = await client.get(`${FIELD}/personal`, auth);
    T('Sync: personal sincronizado visible', assert(sList, Array.isArray(sList.body) && sList.body.some((x) => x.id === syncId)), '');
    const sBad = await client.post(`${FIELD}/sync`, { operaciones: [{ tipo: 'entidad_inexistente', payload: { id: 'x' } }] }, auth);
    const sBadOk = Array.isArray(sBad.body) && sBad.body[0]?.ok === false;
    T('Sync: tipo no soportado → ok:false', assert(sBad, sBadOk, JSON.stringify(sBad.body)), JSON.stringify(sBad.body));
    const sForbid = await client.post(`${FIELD}/sync`, { operaciones: [{ tipo: 'personal', payload: { id: 'x', nombre: 'y' } }] }, authC);
    T('Sync: rol comprador → 403', assert(sForbid, sForbid.status === 403), '');

    // Limpieza de lo creado sin endpoint de borrado
    await client.delete(`${FIELD}/personal/${aPid}`, auth).catch(() => {});
    await client.delete(`${FIELD}/rutas/${rId2}`, auth).catch(() => {});
  } catch (e) {
    errores.push(`Suite 18: ${e.message}`);
  } finally {
    await limpiar();
  }

  const pasados = resultados.filter((r) => r.pass).length;
  return {
    test: 'Field Service (backend móvil / app-test)',
    pass: errores.length === 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}
