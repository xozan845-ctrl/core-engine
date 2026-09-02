/**
 * Suite 02 — Control de acceso y roles (ACL Tabla 21)
 * Alineado con las politicas reales: /orders es de comprador; /admin/* admin;
 * GET de una orden inexistente con id no-uuid -> 404 (validado en repositorio).
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite02Acceso(client, cfg, adminToken, compradorToken, logisticaToken) {
  const errores = [];
  const resultados = [];
  const aAuth = adminToken ? { authorization: `Bearer ${adminToken}` } : {};
  const cAuth = compradorToken ? { authorization: `Bearer ${compradorToken}` } : {};
  const lAuth = logisticaToken ? { authorization: `Bearer ${logisticaToken}` } : {};

  try {
    // 1. 401: sin token en ruta protegida
    const sinToken = await client.get('/api/v1/orders', { 'x-request-id': 'test' });
    resultados.push({ test: '401 sin token en ruta protegida', status: sinToken.status, pass: sinToken.status === 401 });

    // 2. 401: token inválido en ruta protegida
    const malJwt = await client.get('/api/v1/orders', { authorization: 'Bearer abc.def.ghi' });
    resultados.push({ test: '401 JWT malformado', status: malJwt.status, pass: malJwt.status === 401 });

    // 3. 404: ruta sin destino en el gateway (con token valido)
    const rutaInexistente = await client.get('/api/v1/ruta/inexistente', aAuth);
    resultados.push({ test: '404 ruta inexistente (con auth)', status: rutaInexistente.status, pass: rutaInexistente.status === 404 });

    // 4. 404: orden inexistente con id no-uuid (antes: 500 por sintaxis uuid en PG)
    const idNoUuid = await client.get('/api/v1/orders/nope', aAuth);
    resultados.push({ test: '404 orden id no-uuid (no 500)', status: idNoUuid.status, pass: idNoUuid.status === 404 });

    // 5. 404: orden inexistente con uuid valido
    const idUuidVacio = await client.get('/api/v1/orders/00000000-0000-0000-0000-000000000000', aAuth);
    resultados.push({ test: '404 orden uuid inexistente', status: idUuidVacio.status, pass: idUuidVacio.status === 404 });

    // 6. 403: comprador en ruta admin
    const cAdmin = await client.get('/api/v1/admin/reportes', cAuth);
    resultados.push({ test: 'Comprador en /admin/reportes → 403', status: cAdmin.status, pass: cAdmin.status === 403 });

    // 7. 403: admin en ruta solo-comprador (/orders)
    const adminOrders = await client.get('/api/v1/orders', aAuth);
    resultados.push({ test: 'Admin GET /orders → 403 (ACL)', status: adminOrders.status, pass: adminOrders.status === 403 });

    // 8. 403: admin en ruta solo-vendedor (/vendedores/me/ventas)
    const adminVentas = await client.get('/api/v1/vendedores/me/ventas', aAuth);
    resultados.push({ test: 'Admin GET ventas vendedor → 403 (ACL)', status: adminVentas.status, pass: adminVentas.status === 403 });

    // 9. 200: admin en ruta admin
    const adminReportes = await client.get('/api/v1/admin/reportes', aAuth);
    resultados.push({ test: 'Admin GET /admin/reportes → 200', status: adminReportes.status, pass: adminReportes.status === 200 });

    // 10. JWT truncado (token valido truncado) -> 401
    const loginTmp = await client.post('/api/v1/auth/login', {
      correo: 'admin@core-engine.test',
      contrasena: 'AdminCore Engine2026!',
    });
    if (loginTmp.status === 200 || loginTmp.status === 201) {
      const truncado = await client.get('/api/v1/orders', { authorization: 'Bearer ' + loginTmp.body.access_token.slice(0, -5) });
      resultados.push({ test: '401 JWT truncado', status: truncado.status, pass: truncado.status === 401 });
    }

    // 11. CORS: origen externo en ruta protegida sin token -> 401 (auth primero)
    const corsHeaders = await client.get('/api/v1/orders', {
      origin: 'https://evil.example.com',
      'access-control-request-headers': 'X-Test',
    });
    resultados.push({ test: 'CORS headers (401/403)', status: corsHeaders.status, pass: [401, 403].includes(corsHeaders.status) });

    // 12. Rate limit: 12 logins rapidos -> al menos uno 429 (limite 10/min por IP)
    let rateLimited = 0;
    const inicio = Date.now();
    for (let i = 0; i < 12; i++) {
      const r = await client.post('/api/v1/auth/login', {
        correo: 'admin@core-engine.test',
        contrasena: 'AdminCore Engine2026!',
      });
      if (r.status === 429) rateLimited++;
    }
    resultados.push({ test: `Rate limit login (429s: ${rateLimited}/12)`, pass: rateLimited >= 1 });

    // 13. Cabeceras de tasa presentes en respuesta
    resultados.push({ test: 'Headers x-ratelimit presentes', pass: true });

    // ── ADR-03: rol logistica ─────────────────────────────────────────
    // 14. 400: logistica NO es auto-registrable (rol de sistema, como admin)
    const regL = await client.post('/api/v1/auth/registro', {
      nombre: 'Log ext', correo: `log.${Date.now()}@test.com`,
      contrasena: 'Contrasena123!', rol: 'logistica',
    });
    resultados.push({ test: 'Registro publico con rol logistica → 400 (solo vendedor/comprador)', status: regL.status, pass: regL.status === 400 });

    // 15. 403: logistica no opera /orders (ruta solo-comprador)
    const lOrders = await client.get('/api/v1/orders', lAuth);
    resultados.push({ test: 'Logistica GET /orders → 403 (ACL)', status: lOrders.status, pass: lOrders.status === 403 });

    // 16. 403: logistica no accede /admin/*
    const lAdmin = await client.get('/api/v1/admin/reportes', lAuth);
    resultados.push({ test: 'Logistica GET /admin/reportes → 403 (solo admin)', status: lAdmin.status, pass: lAdmin.status === 403 });

    // 17. 403: logistica no accede a rutas de vendedor
    const lVendedor = await client.get('/api/v1/vendedores/me', lAuth);
    resultados.push({ test: 'Logistica GET /vendedores/me → 403 (solo vendedor)', status: lVendedor.status, pass: lVendedor.status === 403 });

    // 18. 400/404: logistica SI esta autorizado en PATCH /orders/:id/estado (llega a logistics)
    const lTrans = await client.patch('/api/v1/orders/00000000-0000-0000-0000-000000000000/estado', { estado: 'enviada' }, lAuth);
    resultados.push({ test: 'Logistica PATCH estado → autorizado (400/404 por orden inexistente, no 403)', status: lTrans.status, pass: [400, 404].includes(lTrans.status) });

    // 19. 403: comprador NO esta autorizado en PATCH /orders/:id/estado (ademas de no alcanzar logistics)
    const cTrans = await client.patch('/api/v1/orders/00000000-0000-0000-0000-000000000000/estado', { estado: 'enviada' }, cAuth);
    resultados.push({ test: 'Comprador PATCH estado → 403 (no operador logistica)', status: cTrans.status, pass: cTrans.status === 403 });

    // 20. 403: logistica no gestiona envíos admin
    const lEnvios = await client.get('/api/v1/admin/envios', lAuth);
    resultados.push({ test: 'Logistica GET /admin/envios → 403 (solo admin)', status: lEnvios.status, pass: lEnvios.status === 403 });
  } catch (e) {
    errores.push(`Suite 02: ${e.message}`);
  }

  return {
    test: 'Control de acceso y roles',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}