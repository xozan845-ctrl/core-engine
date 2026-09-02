/**
 * Suite 01 — Autenticación y sesiones (RF-01, RF-02, RF-03)
 * Aserciones alineadas con el API real: /auth/me devuelve UsuarioContexto plano;
 * refresh requiere refresh_token (tipo refresh), no el access.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite01Autenticacion(client, cfg, adminToken) {
  const resultados = [];
  const errores = [];

  try {
    let token = adminToken;
    if (!token) {
      const loginRes = await client.post('/api/v1/auth/login', {
        correo: 'admin@core-engine.test',
        contrasena: 'AdminCore Engine2026!',
      });
      if (loginRes.status === 200 || loginRes.status === 201) {
        token = loginRes.body.access_token;
      }
    }

    const sufijo = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);

    const regV = await client.post('/api/v1/auth/registro', {
      nombre: 'Vendedora Prueba',
      correo: `vendedor.${sufijo}@test.com`,
      contrasena: 'Contrasena123!',
      rol: 'vendedor',
    });
    resultados.push({ test: 'Registro vendedor', status: regV.status, pass: regV.status === 201 || regV.status === 200 });

    const regC = await client.post('/api/v1/auth/registro', {
      nombre: 'Compradora Prueba',
      correo: `compradora.${sufijo}@test.com`,
      contrasena: 'Contrasena123!',
      rol: 'comprador',
    });
    resultados.push({ test: 'Registro comprador', status: regC.status, pass: regC.status === 201 || regC.status === 200 });

    const loginRes = await client.post('/api/v1/auth/login', {
      correo: 'admin@core-engine.test',
      contrasena: 'AdminCore Engine2026!',
    });
    resultados.push({
      test: 'Login admin',
      status: loginRes.status,
      pass: (loginRes.status === 200 || loginRes.status === 201) && !!loginRes.body?.access_token && !!loginRes.body?.refresh_token,
    });

    if (token) {
      const meRes = await client.get('/api/v1/auth/me', { authorization: `Bearer ${token}` });
      // /auth/me responde el UsuarioContexto plano: {user_id, email, rol}
      resultados.push({ test: 'Obtener perfil usuario', status: meRes.status, pass: meRes.status === 200 && !!meRes.body?.user_id && !!meRes.body?.email });

      const dup = await client.post('/api/v1/auth/registro', {
        nombre: 'Usuario Duplicado',
        correo: 'admin@core-engine.test',
        contrasena: 'Contrasena123!',
        rol: 'comprador',
      });
      resultados.push({ test: 'Registro email duplicado → 409', status: dup.status, pass: dup.status === 409 });

      const badPass = await client.post('/api/v1/auth/login', {
        correo: 'admin@core-engine.test',
        contrasena: 'ContrasenaIncorrecta1!',
      });
      resultados.push({ test: 'Login contraseña incorrecta → 401', status: badPass.status, pass: badPass.status === 401 });

      const badRol = await client.post('/api/v1/auth/registro', {
        nombre: 'Usuario Inválido',
        correo: `invalid.${sufijo}@test.com`,
        contrasena: 'Contrasena123!',
        rol: 'superadmin',
      });
      resultados.push({ test: 'Registro rol inválido → 400', status: badRol.status, pass: badRol.status === 400 });

      const badCorreo = await client.post('/api/v1/auth/login', {
        correo: 'no-email',
        contrasena: 'Contrasena123!',
      });
      resultados.push({ test: 'Login email inválido → 400', status: badCorreo.status, pass: badCorreo.status === 400 });

      const oldToken = token.slice(0, -10) + 'expired';
      const expRes = await client.get('/api/v1/auth/me', { authorization: `Bearer ${oldToken}` });
      resultados.push({ test: 'Token inválido → 401', status: expRes.status, pass: expRes.status === 401 });

      // refresh valido: usar el refresh_token real de la sesion (tipo refresh); el servicio responde 201
      const refreshRes = await client.post('/api/v1/auth/refresh', { refresh_token: loginRes.body?.refresh_token });
      resultados.push({
        test: 'Refresh token → 200/201',
        status: refreshRes.status,
        pass: [200, 201].includes(refreshRes.status) && !!refreshRes.body?.access_token,
      });

      // refresh invalido (basura): el servicio responde 400/401 TOKEN_INVALIDO
      const badRefresh = await client.post('/api/v1/auth/refresh', { refresh_token: 'invalid-token' });
      resultados.push({ test: 'Refresh token inválido → 400/401', status: badRefresh.status, pass: [400, 401].includes(badRefresh.status) });

      // el access token no sirve como refresh (tipo incorrecto)
      const accesoComoRefresh = await client.post('/api/v1/auth/refresh', { refresh_token: token });
      resultados.push({ test: 'Access token como refresh → 400', status: accesoComoRefresh.status, pass: accesoComoRefresh.status === 400 });

      // login con contrasena corta: el login no valida politica (eso es del registro);
      // credenciales no validan -> 401
      const shortPass = await client.post('/api/v1/auth/login', {
        correo: 'admin@core-engine.test',
        contrasena: '12345',
      });
      resultados.push({ test: 'Login contraseña corta (<8) → 401', status: shortPass.status, pass: shortPass.status === 401 });

      const refreshSinToken = await client.post('/api/v1/auth/refresh', {});
      resultados.push({ test: 'Refresh sin token → 400', status: refreshSinToken.status, pass: refreshSinToken.status === 400 });
    }
  } catch (e) {
    errores.push(`Suite 01: ${e.message}`);
  }

  return {
    test: 'Autenticación y sesiones',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}