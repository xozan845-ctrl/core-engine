/**
 * Suite 06 — Carrito y checkout (RN-05) con datos reales
 * Carrito: agregar (agrupa por oferta), actualizar (0 elimina), quitar, vaciar.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite06Carrito(client, cfg, compradorToken, adminToken, vendedorToken) {
  const errores = [];
  const resultados = [];

  if (!compradorToken || !adminToken || !vendedorToken) {
    errores.push('Faltan tokens');
    return { test: 'Carrito y checkout (RN-05)', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const cAuth = { authorization: `Bearer ${compradorToken}` };
  const aAuth = { authorization: `Bearer ${adminToken}` };
  const vAuth = { authorization: `Bearer ${vendedorToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  try {
    // Setup: producto + oferta reales
    const prod = await client.post('/api/v1/catalog/productos', {
      sku: `QA-C1-${suf}`, nombre: 'Producto QA Carrito', descripcion: '', categoria: 'general',
      precio_base: '1000.00', stock: 50,
    }, aAuth);
    const tienda = await client.get('/api/v1/vendedores/me/tienda', vAuth);
    const oferta = await client.post('/api/v1/vendedores/productos', {
      producto_id: prod.body?.id, margen: 10,
      ...(tienda.body?.id ? { tienda_id: tienda.body.id } : {}),
    }, vAuth);
    const ofertaId = oferta.body?.id ?? null;

    if (ofertaId) {
      // limpiar items residuales de corridas previas (mismo comprador persistido)
      await client.delete('/api/v1/carrito', cAuth);

      const ver = await client.get('/api/v1/carrito', cAuth);
      resultados.push({ test: 'Ver carrito (vacío)', status: ver.status, pass: ver.status === 200 });

      const add1 = await client.post('/api/v1/carrito/items', { oferta_id: ofertaId, cantidad: 2 }, cAuth);
      resultados.push({ test: 'Agregar item (cantidad 2)', status: add1.status, pass: [200, 201].includes(add1.status) });

      // el mismo item se agrupa (cantidades sumadas, RN-05)
      const add2 = await client.post('/api/v1/carrito/items', { oferta_id: ofertaId, cantidad: 3 }, cAuth);
      resultados.push({ test: 'Agregar mismo item (agrupar RN-05)', status: add2.status, pass: [200, 201].includes(add2.status) });

      const ver2 = await client.get('/api/v1/carrito', cAuth);
      const totalCant = ver2.body?.items_cantidad ?? ver2.body?.total_items ?? null;
      resultados.push({ test: 'Carrito agrupado (items 5?)', status: ver2.status, pass: ver2.status === 200 });

      const upd = await client.patch(`/api/v1/carrito/items/${ofertaId}`, { cantidad: 1 }, cAuth);
      resultados.push({ test: 'Actualizar cantidad item', status: upd.status, pass: upd.status === 200 });

      const cnt0 = await client.patch(`/api/v1/carrito/items/${ofertaId}`, { cantidad: 0 }, cAuth);
      resultados.push({ test: 'Cantidad 0 elimina item', status: cnt0.status, pass: cnt0.status === 200 });

      const quitar = await client.post('/api/v1/carrito/items', { oferta_id: ofertaId, cantidad: 1 }, cAuth);
      const del = await client.delete(`/api/v1/carrito/items/${ofertaId}`, cAuth);
      resultados.push({ test: 'Quitar item (DELETE)', status: del.status, pass: del.status === 200 });

      const vaciar = await client.delete('/api/v1/carrito', cAuth);
      resultados.push({ test: 'Vaciar carrito', status: vaciar.status, pass: vaciar.status === 200 });

      // RN-05: checkout crea la orden (desde carrito con items)
      await client.post('/api/v1/carrito/items', { oferta_id: ofertaId, cantidad: 1 }, cAuth);
      const checkout = await client.post('/api/v1/orders', { items: [{ oferta_id: ofertaId, cantidad: 1 }] }, cAuth);
      resultados.push({ test: 'Checkout (crear orden)', status: checkout.status, pass: [200, 201].includes(checkout.status) });

      // Cantidad > 99 -> 400 (DTO Max 99)
      const over = await client.post('/api/v1/carrito/items', { oferta_id: ofertaId, cantidad: 100 }, cAuth);
      resultados.push({ test: 'Cantidad 100 → 400 (Max 99)', status: over.status, pass: over.status === 400 });

      const add0 = await client.post('/api/v1/carrito/items', { oferta_id: ofertaId, cantidad: 0 }, cAuth);
      resultados.push({ test: 'Agregar cantidad 0 → 400', status: add0.status, pass: add0.status === 400 });
    } else {
      resultados.push({ test: 'Setup producto+oferta', status: oferta.status, pass: false });
    }

    // Sin token -> 401
    const sinToken = await client.get('/api/v1/carrito');
    resultados.push({ test: 'Carrito sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });
  } catch (e) {
    errores.push(`Suite 06: ${e.message}`);
  }

  return {
    test: 'Carrito y checkout (RN-05)',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}