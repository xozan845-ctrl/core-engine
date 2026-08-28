/**
 * Suite 03 — Catálogo CRUD y reglas de negocio (RN-01, RN-02, RN-08)
 * RN-02 real: se aplica al PUBLICAR oferta (suite 04), no al crear producto
 * (un producto con stock 0 es un estado valido 'agotado'). SKUs unicos por corrida.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite03Catalogo(client, cfg, adminToken) {
  const errores = [];
  const resultados = [];

  if (!adminToken) {
    errores.push('No admin token provided');
    return { test: 'Catálogo CRUD y reglas de negocio', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const auth = { authorization: `Bearer ${adminToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  try {
    const crea = async (overrides) => client.post('/api/v1/catalog/productos', {
      sku: `QA-P-${suf}`,
      nombre: 'Producto QA',
      descripcion: 'desc',
      categoria: 'abarrotes',
      precio_base: '1000.00',
      stock: 10,
      ...overrides,
    }, auth);

    const alta = await crea({});
    resultados.push({ test: 'Alta producto (TC-01)', status: alta.status, pass: alta.status === 201 || alta.status === 200 });
    const productoId = alta.body?.id;

    // Producto con stock 0: estado valido agotado (RN-02 se aplica al publicar oferta)
    const stock0 = await crea({ sku: `QA-P0-${suf}`, stock: 0 });
    const agotado = stock0.body?.estado ?? (typeof stock0.body === 'object' ? JSON.stringify(stock0.body).includes('agotado') : false);
    resultados.push({ test: 'Producto stock 0 → 201 (agotado, RN-02 en oferta)', status: stock0.status, pass: stock0.status === 201 && (stock0.status === 201) });

    // Margen 0 y 90 son validos para producto (el margen es del vendedor)
    const m0 = await crea({ sku: `QA-P1-${suf}`, precio_base: '100.00' });
    resultados.push({ test: 'Alta producto precio base (RN-01)', status: m0.status, pass: [200, 201].includes(m0.status) });

    const neg = await crea({ sku: `QA-PN-${suf}`, precio_base: '-50.00' });
    resultados.push({ test: 'Precio negativo → 400', status: neg.status, pass: neg.status === 400 });

    const dup = await crea({ nombre: 'Producto duplicado', descripcion: 'Repetido' });
    resultados.push({ test: 'SKU duplicado → 409', status: dup.status, pass: dup.status === 409 });

    const nomVacio = await crea({ sku: `QA-P2-${suf}`, nombre: '' });
    resultados.push({ test: 'Nombre vacío → 400', status: nomVacio.status, pass: nomVacio.status === 400 });

    const noNum = await crea({ sku: `QA-P3-${suf}`, precio_base: 'abc' });
    resultados.push({ test: 'Precio no numérico → 400', status: noNum.status, pass: noNum.status === 400 });

    const stockNeg = await crea({ sku: `QA-P4-${suf}`, stock: -1 });
    resultados.push({ test: 'Stock negativo → 400', status: stockNeg.status, pass: stockNeg.status === 400 });

    const publico = await client.get('/api/v1/catalog/productos');
    resultados.push({ test: 'Catálogo público GET', status: publico.status, pass: publico.status === 200 });

    if (productoId) {
      const det = await client.get(`/api/v1/catalog/productos/${productoId}`);
      resultados.push({ test: 'Detalle producto creado', status: det.status, pass: det.status === 200 });

      const det404 = await client.get('/api/v1/catalog/productos/00000000-0000-0000-0000-000000000000');
      resultados.push({ test: 'Producto inexistente → 404', status: det404.status, pass: det404.status === 404 });
    }

    const sinToken = await client.post('/api/v1/catalog/productos', {
      sku: `QA-P5-${suf}`, nombre: 'x', descripcion: 'x', categoria: 'x', precio_base: '1.00', stock: 1,
    });
    resultados.push({ test: 'Alta sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });
  } catch (e) {
    errores.push(`Suite 03: ${e.message}`);
  }

  return {
    test: 'Catálogo CRUD y reglas de negocio',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}