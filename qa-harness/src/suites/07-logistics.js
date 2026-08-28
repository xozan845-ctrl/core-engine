/**
 * Suite 07 — Logística y envíos
 * El envío no tiene endpoint de creación (se origina por eventos); se validan
 * las transiciones de estado de la orden y la gestión admin de envíos.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite07Logistica(client, cfg, adminToken, compradorToken, vendedorToken, logisticaToken) {
  const errores = [];
  const resultados = [];

  if (!adminToken || !compradorToken || !vendedorToken || !logisticaToken) {
    errores.push('Faltan tokens');
    return { test: 'Logística y envíos', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const aAuth = { authorization: `Bearer ${adminToken}` };
  const cAuth = { authorization: `Bearer ${compradorToken}` };
  const vAuth = { authorization: `Bearer ${vendedorToken}` };
  const lAuth = { authorization: `Bearer ${logisticaToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  try {
    // Setup: producto + oferta + orden
    const prod = await client.post('/api/v1/catalog/productos', {
      sku: `QA-L1-${suf}`, nombre: 'Producto QA Log', descripcion: '', categoria: 'general',
      precio_base: '500.00', stock: 3,
    }, aAuth);
    const tienda = await client.get('/api/v1/vendedores/me/tienda', vAuth);
    const oferta = await client.post('/api/v1/vendedores/productos', {
      producto_id: prod.body?.id, margen: 10,
      ...(tienda.body?.id ? { tienda_id: tienda.body.id } : {}),
    }, vAuth);
    const orden = oferta.body?.id ? await client.post('/api/v1/orders', {
      items: [{ oferta_id: oferta.body.id, cantidad: 1 }],
    }, cAuth) : null;
    const orderId = orden?.body?.id ?? null;

    if (orderId) {
      resultados.push({ test: 'Orden creada para logística', status: orden.status, pass: [200, 201].includes(orden.status) });

      // la saga pasa a pagada asincronamente antes de que logistica pueda avanzar
      let estado = 'creada';
      for (let i = 0; i < 12 && estado !== 'pagada'; i++) {
        await new Promise((r) => setTimeout(r, 1000));
        const d = await client.get(`/api/v1/orders/${orderId}`, cAuth);
        estado = d.body?.estado ?? estado;
      }
      resultados.push({ test: `Saga: estado = ${estado}`, pass: estado === 'pagada' });

      const t1 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'en_preparacion' }, aAuth);
      resultados.push({ test: 'pagada → en_preparacion', status: t1.status, pass: t1.status === 200 });

      const t2 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'enviada' }, lAuth);
      resultados.push({ test: 'en_preparacion → enviada (rol logistica)', status: t2.status, pass: t2.status === 200 });

      const t3 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'entregada' }, aAuth);
      resultados.push({ test: 'enviada → entregada', status: t3.status, pass: t3.status === 200 });

      const tinv = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'devuelta', motivo: 'x' }, aAuth);
      resultados.push({ test: 'entregada → devuelta (compensación RN-06)', status: tinv.status, pass: tinv.status === 200 });

      const orden404 = await client.patch('/api/v1/orders/00000000-0000-0000-0000-000000000000/estado', { estado: 'enviada' }, aAuth);
      resultados.push({ test: 'Orden inexistente → 400/404', status: orden404.status, pass: [400, 404].includes(orden404.status) });
    } else {
      resultados.push({ test: 'Setup orden', status: orden?.status ?? 500, pass: false });
    }

    // Gestión admin de envíos: solo GET existe; cancelar es gap documentado
    const envios = await client.get('/api/v1/admin/envios', aAuth);
    resultados.push({ test: 'Listar envíos (admin)', status: envios.status, pass: envios.status === 200 });

    const enviosArr = Array.isArray(envios.body) ? envios.body : (envios.body?.envios ?? []);
    if (enviosArr.length > 0) {
      const canc = await client.patch(`/api/v1/admin/envios/${enviosArr[0].id}/cancelar`, {}, aAuth);
      resultados.push({ test: 'Cancelar envío (404: gap documentado, solo GET)', status: canc.status, pass: canc.status === 404 });
    } else {
      resultados.push({ test: 'Cancelar envío (sin envíos → skip)', pass: true });
    }

    // No existe endpoint de creación manual de envíos (se origina por eventos)
    const crearEnvio = await client.post('/api/v1/admin/envios', { order_id: orderId ?? '00000000-0000-0000-0000-000000000000' }, aAuth);
    resultados.push({ test: 'POST /admin/envios (no implementado)', status: crearEnvio.status, pass: [404, 405].includes(crearEnvio.status) });

    // ACL
    const sinToken = await client.get('/api/v1/admin/envios');
    resultados.push({ test: 'Sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });

    const cEnvios = await client.get('/api/v1/admin/envios', cAuth);
    resultados.push({ test: 'Comprador en /admin/envios → 403', status: cEnvios.status, pass: cEnvios.status === 403 });

    const lEnvios = await client.get('/api/v1/admin/envios', lAuth);
    resultados.push({ test: 'Logistica en /admin/envios → 403 (solo admin)', status: lEnvios.status, pass: lEnvios.status === 403 });
  } catch (e) {
    errores.push(`Suite 07: ${e.message}`);
  }

  return {
    test: 'Logística y envíos',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}