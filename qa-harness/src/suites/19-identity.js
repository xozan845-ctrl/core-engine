/**
 * Suite 19 · identity-admin (auth + gestión de usuarios)
 *
 * Cubre el servicio de identidad (sustituto de Firebase Auth) expuesto en
 * /api/v1/auth y /api/v1/usuarios:
 *  - registro (Tabla 21), login, refresh, me
 *  - alta administrativa (crear-usuario), vincular-personal
 *  - cambiar-contrasena, restablecer-contrasena
 *  - listado de usuarios
 *  - validaciones y RBAC/observación de brechas.
 *
 * Los endpoints son públicos o requieren token según el controlador; donde el
 * controlador NO exige token se documenta como hallazgo (observación).
 */
export async function suite19Identity({ client, pg, config }) {
  function assert(res, cond, detail) {
    return { status: cond ? res.status : `${res.status}❌`, pass: !!cond, detail };
  }
  const errores = [];
  const resultados = [];
  const T = (test, res, detail = '') => {
    resultados.push({ test, status: res.status, pass: res.pass });
    if (!res.pass) errores.push(`${test} :: ${detail || res.status}`);
  };
  // El gateway limita POST /auth/login a 10/min por IP (anti fuerza bruta).
  // Espaciamos los logins >=7s y respetamos Retry-After para no disparar 429.
  const MIN_GAP_MS = 7000;
  let ultimoLogin = 0;
  const sleep = (ms) => new Promise((res) => setTimeout(res, ms));
  const loginRaw = async (body) => {
    const espera = ultimoLogin + MIN_GAP_MS - Date.now();
    if (espera > 0) await sleep(espera);
    for (let i = 0; i < 5; i++) {
      const r = await client.post('/api/v1/auth/login', body);
      ultimoLogin = Date.now();
      if (r.status !== 429) return r;
      const ra = Number(r.body?.detalles?.reintente_en_seg) || 65;
      await sleep(Math.min(ra, 65) * 1000);
    }
    return client.post('/api/v1/auth/login', body);
  };
  const login = (correo, pass) => loginRaw({ correo, contrasena: pass });
  const adminEmail = config.adminEmail || 'admin@core-engine.test';
  const adminPassword = config.adminPassword || 'AdminCore Engine2026!';
  const suf = (config.sufijo || 'qa') + '-' + Date.now().toString().slice(-6);

  const loginAdmin = await login(adminEmail, adminPassword);
  const adminToken = loginAdmin.body?.access_token;
  const authA = { authorization: `Bearer ${adminToken}` };
  T('admin: login exitoso', assert(loginAdmin, loginAdmin.status === 200 && !!adminToken), JSON.stringify(loginAdmin.status));

  // ── registro (Tabla 21) ──────────────────────────────────────────────────
  const vCorreo = `vend.${suf}@test.com`;
  const regV = await client.post('/api/v1/auth/registro', { nombre: 'Vendedor QA', correo: vCorreo, contrasena: 'Contrasena123!', rol: 'vendedor' });
  T('registro: vendedor válido → 201 + tokens', assert(regV, regV.status === 201 && !!regV.body?.access_token && !!regV.body?.refresh_token), JSON.stringify(regV.status));
  const cCorreo = `comp.${suf}@test.com`;
  const regC = await client.post('/api/v1/auth/registro', { nombre: 'Comprador QA', correo: cCorreo, contrasena: 'Contrasena123!', rol: 'comprador' });
  T('registro: comprador válido → 201', assert(regC, regC.status === 201), JSON.stringify(regC.status));
  const dup = await client.post('/api/v1/auth/registro', { nombre: 'Dupe', correo: vCorreo, contrasena: 'Contrasena123!', rol: 'vendedor' });
  T('registro: correo duplicado → 409', assert(dup, dup.status === 409), JSON.stringify(dup.status));
  const badRol = await client.post('/api/v1/auth/registro', { nombre: 'X', correo: `x.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'admin' });
  T('registro: rol no registrable (admin) → 400', assert(badRol, badRol.status === 400), JSON.stringify(badRol.status));
  const shortPass = await client.post('/api/v1/auth/registro', { nombre: 'X', correo: `sp.${suf}@test.com`, contrasena: 'corto', rol: 'vendedor' });
  T('registro: contraseña corta → 400', assert(shortPass, shortPass.status === 400), JSON.stringify(shortPass.status));
  const badCorreo = await client.post('/api/v1/auth/registro', { nombre: 'X', correo: 'no-es-correo', contrasena: 'Contrasena123!', rol: 'vendedor' });
  T('registro: correo inválido → 400', assert(badCorreo, badCorreo.status === 400), JSON.stringify(badCorreo.status));

  // ── login ────────────────────────────────────────────────────────────────
  const badPass = await login(adminEmail, 'incorrecta');
  T('login: contraseña incorrecta → 401', assert(badPass, badPass.status === 401), JSON.stringify(badPass.status));
  const badUser = await login(`noexiste.${suf}@test.com`, 'Contrasena123!');
  T('login: usuario inexistente → 401', assert(badUser, badUser.status === 401), JSON.stringify(badUser.status));
  const noBody = await loginRaw({});
  T('login: cuerpo vacío → 400', assert(noBody, noBody.status === 400), JSON.stringify(noBody.status));

  // ── me ────────────────────────────────────────────────────────────────────
  const meSin = await client.get('/api/v1/auth/me', {});
  T('me: sin token → 401', assert(meSin, meSin.status === 401), JSON.stringify(meSin.status));
  const meCon = await client.get('/api/v1/auth/me', authA);
  T('me: con token → 200 + contexto', assert(meCon, meCon.status === 200 && !!meCon.body?.user_id && !!meCon.body?.rol), JSON.stringify(meCon.status));

  // ── refresh ───────────────────────────────────────────────────────────────
  const refreshOk = await client.post('/api/v1/auth/refresh', { refresh_token: loginAdmin.body?.refresh_token });
  T('refresh: token válido → 200/201 + tokens', assert(refreshOk, [200, 201].includes(refreshOk.status) && !!refreshOk.body?.access_token && !!refreshOk.body?.refresh_token), JSON.stringify(refreshOk.status));
  const refreshBad = await client.post('/api/v1/auth/refresh', { refresh_token: 'token-invalido' });
  T('refresh: token inválido → 400/401', assert(refreshBad, [400, 401].includes(refreshBad.status)), JSON.stringify(refreshBad.status));
  const refreshEmpty = await client.post('/api/v1/auth/refresh', {});
  T('refresh: sin token → 400', assert(refreshEmpty, refreshEmpty.status === 400), JSON.stringify(refreshEmpty.status));

  // ── crear-usuario (alta administrativa) ───────────────────────────────────
  const cuv = await client.post('/api/v1/auth/crear-usuario', { nombre: 'Alta V', correo: `altav.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'vendedor' }, authA);
  T('crear-usuario: vendedor → 201 + id', assert(cuv, cuv.status === 201 && !!cuv.body?.id), JSON.stringify(cuv.status));
  const cul = await client.post('/api/v1/auth/crear-usuario', { nombre: 'Alta L', correo: `altal.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'logistica' }, authA);
  T('crear-usuario: logistica sin tenant → 400 (TENANT_REQUERIDO)', assert(cul, cul.status === 400 && (cul.body?.codigo === 'TENANT_REQUERIDO' || cul.body?.codigo === 'SOLICITUD_INVALIDA')), JSON.stringify({ s: cul.status, c: cul.body?.codigo }));
  const cult = await client.post('/api/v1/auth/crear-usuario', { nombre: 'Alta LT', correo: `altalt.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'logistica', tenant_id: '11111111-1111-1111-1111-111111111111' }, authA);
  T('crear-usuario: logistica con tenant → 201', assert(cult, cult.status === 201 && !!cult.body?.id), JSON.stringify(cult.status));
  const cuBadRol = await client.post('/api/v1/auth/crear-usuario', { nombre: 'X', correo: `xr.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'rol_inexistente' }, authA);
  T('crear-usuario: rol inválido → 400', assert(cuBadRol, cuBadRol.status === 400), JSON.stringify(cuBadRol.status));
  const cuDup = await client.post('/api/v1/auth/crear-usuario', { nombre: 'Dupe', correo: `altav.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'vendedor' }, authA);
  T('crear-usuario: correo duplicado → 409', assert(cuDup, cuDup.status === 409), JSON.stringify(cuDup.status));
  // RBAC: crear-usuario exige token de administrador.
  const cuSinToken = await client.post('/api/v1/auth/crear-usuario', { nombre: 'SinTok', correo: `sintok.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'vendedor' }, {});
  T('crear-usuario: SIN token → 401 (authz requerida)', assert(cuSinToken, cuSinToken.status === 401), JSON.stringify(cuSinToken.status));

  // ── cambiar-contrasena ─────────────────────────────────────────────────────
  const ccCorreo = `cc.${suf}@test.com`;
  const ccReg = await client.post('/api/v1/auth/registro', { nombre: 'CC', correo: ccCorreo, contrasena: 'PassUno123!', rol: 'vendedor' });
  const ccToken = ccReg.body?.access_token;
  const ccAuth = { authorization: `Bearer ${ccToken}` };
  // Un comprador (no admin) debe recibir 403 al intentar crear usuarios.
  const cuNoAdmin = await client.post('/api/v1/auth/crear-usuario', { nombre: 'NoAdmin', correo: `noadmin.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'vendedor' }, ccAuth);
  T('crear-usuario: rol no admin → 403 (Forbidden)', assert(cuNoAdmin, cuNoAdmin.status === 403), JSON.stringify(cuNoAdmin.status));
  const ccSin = await client.post('/api/v1/auth/cambiar-contrasena', { actual: 'x', nueva: 'y' }, {});
  T('cambiar-contrasena: sin token → 401', assert(ccSin, ccSin.status === 401), JSON.stringify(ccSin.status));
  const ccWrong = await client.post('/api/v1/auth/cambiar-contrasena', { actual: 'mala', nueva: 'PassDos456!' }, ccAuth);
  T('cambiar-contrasena: actual incorrecta → 400/401', assert(ccWrong, [400, 401].includes(ccWrong.status)), JSON.stringify(ccWrong.status));
  const ccShort = await client.post('/api/v1/auth/cambiar-contrasena', { actual: 'PassUno123!', nueva: 'corta' }, ccAuth);
  T('cambiar-contrasena: nueva corta → 400', assert(ccShort, ccShort.status === 400), JSON.stringify(ccShort.status));
  const ccOk = await client.post('/api/v1/auth/cambiar-contrasena', { actual: 'PassUno123!', nueva: 'PassDos456!' }, ccAuth);
  T('cambiar-contrasena: válido → 200/201 ok', assert(ccOk, [200, 201].includes(ccOk.status) && ccOk.body?.ok === true), JSON.stringify(ccOk.status));
  const loginVieja = await login(ccCorreo, 'PassUno123!');
  T('cambiar-contrasena: login con contraseña vieja → 401', assert(loginVieja, loginVieja.status === 401), JSON.stringify(loginVieja.status));
  const loginNueva = await login(ccCorreo, 'PassDos456!');
  T('cambiar-contrasena: login con contraseña nueva → 200', assert(loginNueva, loginNueva.status === 200), JSON.stringify(loginNueva.status));

  // ── restablecer-contrasena ─────────────────────────────────────────────────
  const rcOk = await client.post('/api/v1/auth/restablecer-contrasena', { correo: vCorreo });
  T('restablecer: correo existente → 200/201 ok (no revela)', assert(rcOk, [200, 201].includes(rcOk.status) && rcOk.body?.ok === true), JSON.stringify(rcOk.status));
  const rcBad = await client.post('/api/v1/auth/restablecer-contrasena', { correo: 'no-correo' });
  T('restablecer: correo inválido → 400', assert(rcBad, rcBad.status === 400), JSON.stringify(rcBad.status));
  const rcUnknown = await client.post('/api/v1/auth/restablecer-contrasena', { correo: `desconocido.${suf}@test.com` });
  T('restablecer: correo desconocido → 200/201 ok (no revela)', assert(rcUnknown, [200, 201].includes(rcUnknown.status) && rcUnknown.body?.ok === true), JSON.stringify(rcUnknown.status));

  // ── vincular-personal ──────────────────────────────────────────────────────
  const vpSin = await client.post('/api/v1/auth/vincular-personal', { personal_id: '22222222-2222-2222-2222-222222222222' }, {});
  T('vincular-personal: sin token → 401', assert(vpSin, vpSin.status === 401), JSON.stringify(vpSin.status));
  const vpOk = await client.post('/api/v1/auth/vincular-personal', { personal_id: '33333333-3333-3333-3333-333333333333', nombre: 'Perfil Vinculado' }, ccAuth);
  T('vincular-personal: válido → 200/201 ok', assert(vpOk, [200, 201].includes(vpOk.status) && vpOk.body?.ok === true), JSON.stringify(vpOk.status));
  const vpEmpty = await client.post('/api/v1/auth/vincular-personal', {}, ccAuth);
  T('vincular-personal: personal_id faltante → 400', assert(vpEmpty, vpEmpty.status === 400), JSON.stringify(vpEmpty.status));

  // ── listado de usuarios ────────────────────────────────────────────────────
  const list = await client.get('/api/v1/usuarios', authA);
  T('usuarios: listar (admin) → 200 array', assert(list, list.status === 200 && Array.isArray(list.body)), JSON.stringify(list.status));

  const pasados = resultados.filter((r) => r.pass).length;
  return {
    test: 'Identity · auth + gestión de usuarios',
    pass: errores.length === 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}
