/**
 * Suite 11 — Fuzzing y validación de entradas agresiva
 * Rutas publicas sin token + rutas protegidas con token real.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite11Fuzzing(client, cfg, adminToken, vendedorToken) {
  const errores = [];
  const resultados = [];
  const aAuth = adminToken ? { authorization: `Bearer ${adminToken}` } : {};
  const vAuth = vendedorToken ? { authorization: `Bearer ${vendedorToken}` } : {};
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  const acepta = (status, lista) => lista.includes(status);

  try {
    // 1-3: fuzz sobre login publico (acepta 400/401/429 segun tasa)
    const malJson = await client.post('/api/v1/auth/login', '{correo: "test"}', { 'Content-Type': 'application/json' });
    resultados.push({ test: 'JSON malformado → 400', status: malJson.status, pass: acepta(malJson.status, [400, 429]) });

    const sqli = await client.post('/api/v1/auth/login', { correo: "admin' OR '1'='1", contrasena: 'AdminBodegaHub2026!' });
    resultados.push({ test: 'SQLi en correo → 400/401', status: sqli.status, pass: acepta(sqli.status, [400, 401, 429]) });

    const proto = await client.post('/api/v1/auth/login', {
      correo: 'test@test.com',
      contrasena: 'test',
      __proto__: { admin: true },
      constructor: { prototype: { admin: true } },
    });
    resultados.push({ test: 'Prototype pollution attempt', status: proto.status, pass: acepta(proto.status, [400, 401, 429]) });

    // 4. array como body (logins realizados → posibles 429)
    const arrayObj = await client.post('/api/v1/auth/login', []);
    resultados.push({ test: 'Array en body login → 400/429', status: arrayObj.status, pass: acepta(arrayObj.status, [400, 429]) });

    // 5-6. registro publico: XSS y nombre vacio
    const xss = await client.post('/api/v1/auth/registro', {
      nombre: '<script>alert(1)</script>',
      correo: `xss.${suf}@test.com`,
      contrasena: 'Contrasena123!',
      rol: 'comprador',
    });
    resultados.push({ test: 'XSS en nombre → 201/400', status: xss.status, pass: acepta(xss.status, [201, 400]) });

    const vacio = await client.post('/api/v1/auth/registro', {
      nombre: '', correo: `vacio.${suf}@test.com`, contrasena: 'Contrasena123!', rol: 'comprador',
    });
    resultados.push({ test: 'Nombre vacío registro → 400', status: vacio.status, pass: vacio.status === 400 });

    // 7-9. fuzz sobre catálogo con token admin (DTO validation)
    const sinToken = await client.post('/api/v1/catalog/productos', {
      sku: `QA-F0-${suf}`, nombre: 'x', descripcion: 'x', categoria: 'x', precio_base: '1.00', stock: 1,
    });
    resultados.push({ test: 'Catálogo sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });

    const tipos = await client.post('/api/v1/catalog/productos', {
      sku: `QA-F1-${suf}`, nombre: 'x', descripcion: 'x', categoria: 'x', precio_base: '1.00', stock: '10',
    }, aAuth);
    resultados.push({ test: 'Tipos incorrectos (stock str) → 400', status: tipos.status, pass: tipos.status === 400 });

    const nulos = await client.post('/api/v1/catalog/productos', {
      sku: null, nombre: 'x', descripcion: 'x', categoria: 'x', precio_base: '1.00', stock: 10,
    }, aAuth);
    resultados.push({ test: 'null en campo requerido → 400', status: nulos.status, pass: nulos.status === 400 });

    // 10. margen fuera de rango con token vendedor
    const rango = await client.post('/api/v1/vendedores/productos', { producto_id: 'x', margen: 91 }, vAuth);
    resultados.push({ test: 'Margen 91 → 400', status: rango.status, pass: rango.status === 400 });

    // 11. producto inexistente publico → 404
    const inexistente = await client.get('/api/v1/catalog/productos/00000000-0000-0000-0000-000000000000');
    resultados.push({ test: 'Producto inexistente → 404', status: inexistente.status, pass: inexistente.status === 404 });
  } catch (e) {
    errores.push(`Suite 11: ${e.message}`);
  }

  return {
    test: 'Fuzzing y validación de entradas agresiva',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}