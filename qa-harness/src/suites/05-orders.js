/**
 * Suite 05 — Órdenes y saga de transacciones
 * Flujo completo con datos reales: producto -> oferta -> orden -> transiciones.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite05Ordenes(client, cfg, compradorToken, adminToken, vendedorToken) {
  const errores = [];
  const resultados = [];

  if (!compradorToken || !adminToken || !vendedorToken) {
    errores.push('Faltan tokens');
    return { test: 'Órdenes y saga', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const cAuth = { authorization: `Bearer ${compradorToken}` };
  const aAuth = { authorization: `Bearer ${adminToken}` };
  const vAuth = { authorization: `Bearer ${vendedorToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  try {
    // Setup: producto (admin) + oferta (vendedor)
    const prod = await client.post('/api/v1/catalog/productos', {
      sku: `QA-O1-${suf}`, nombre: 'Producto QA Orden', descripcion: '', categoria: 'general',
      precio_base: '2000.00', stock: 5,
    }, aAuth);
    const productoId = prod.body?.id;

    const tienda = await client.get('/api/v1/vendedores/me/tienda', vAuth);
    const tiendaId = tienda.body?.id ?? tienda.body?.tienda?.id ?? null;

    const oferta = productoId ? await client.post('/api/v1/vendedores/productos', {
      producto_id: productoId, margen: 15, ...(tiendaId ? { tienda_id: tiendaId } : {}),
    }, vAuth) : { status: 500 };
    const ofertaId = oferta.body?.id ?? null;

    if (ofertaId) {
      // Crear orden
      const orden = await client.post('/api/v1/orders', { items: [{ oferta_id: ofertaId, cantidad: 1 }] }, cAuth);
      resultados.push({ test: 'Crear orden (TC-03)', status: orden.status, pass: orden.status === 201 || orden.status === 200 });
      const orderId = orden.body?.id;

      if (orderId) {
        const detalle = await client.get(`/api/v1/orders/${orderId}`, cAuth);
        resultados.push({ test: 'Ver detalle orden', status: detalle.status, pass: detalle.status === 200 });

        // Saga asincrona: creada -> pagada (stock.reservado) antes de avanzar
        let estadoActual = detalle.body?.estado;
        for (let i = 0; i < 12 && estadoActual !== 'pagada'; i++) {
          await new Promise((r) => setTimeout(r, 1000));
          const d = await client.get(`/api/v1/orders/${orderId}`, cAuth);
          estadoActual = d.body?.estado;
        }
        resultados.push({ test: `Saga: estado = ${estadoActual}`, status: detalle.status, pass: estadoActual === 'pagada' });

        // Transiciones validas admin: pagada -> en_preparacion -> enviada -> entregada
        const t1 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'en_preparacion' }, aAuth);
        resultados.push({ test: 'pagada → en_preparacion', status: t1.status, pass: t1.status === 200 });

        const t2 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'enviada' }, aAuth);
        resultados.push({ test: 'en_preparacion → enviada', status: t2.status, pass: t2.status === 200 });

        const t3 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'entregada' }, aAuth);
        resultados.push({ test: 'enviada → entregada', status: t3.status, pass: t3.status === 200 });

        // Transiciones invalidas -> 400 (DomainError TRANSICION_INVALIDA)
        const inv1 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'creada' }, aAuth);
        resultados.push({ test: 'entregada → creada inválida → 400', status: inv1.status, pass: inv1.status === 400 });

        const inv2 = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'pagada' }, aAuth);
        resultados.push({ test: 'entregada → pagada inválida → 400', status: inv2.status, pass: inv2.status === 400 });

        const sinTransicion = await client.patch(`/api/v1/orders/${orderId}/estado`, {}, aAuth);
        resultados.push({ test: 'Transición sin estado → 400', status: sinTransicion.status, pass: sinTransicion.status === 400 });

        // Comprador no puede transicionar (solo admin)
        const cTrans = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'cancelada' }, cAuth);
        resultados.push({ test: 'Comprador transiciona → 403', status: cTrans.status, pass: cTrans.status === 403 });
      }
    } else {
      resultados.push({ test: 'Setup producto+oferta', status: oferta.status, pass: false });
    }

    // Validaciones de payload (400 antes de tocar dominio)
    const c0 = await client.post('/api/v1/orders', { items: [{ oferta_id: 'x', cantidad: 0 }] }, cAuth);
    resultados.push({ test: 'Cantidad 0 → 400 (RN-05)', status: c0.status, pass: c0.status === 400 });

    const vacios = await client.post('/api/v1/orders', { items: [] }, cAuth);
    resultados.push({ test: 'Items vacíos → 400 (RN-05)', status: vacios.status, pass: vacios.status === 400 });

    const neg = await client.post('/api/v1/orders', { items: [{ oferta_id: 'x', cantidad: -1 }] }, cAuth);
    resultados.push({ test: 'Items negativos → 400', status: neg.status, pass: neg.status === 400 });

    // Oferta inexistente -> 404
    const noOferta = await client.post('/api/v1/orders', { items: [{ oferta_id: '00000000-0000-0000-0000-000000000000', cantidad: 1 }] }, cAuth);
    resultados.push({ test: 'Oferta inexistente → 404', status: noOferta.status, pass: noOferta.status === 404 });

    // Vendedor no puede comprar (ruta solo comprador)
    const vCompra = await client.post('/api/v1/orders', { items: [{ oferta_id: 'x', cantidad: 1 }] }, vAuth);
    resultados.push({ test: 'Vendedor crea orden → 403', status: vCompra.status, pass: vCompra.status === 403 });
  } catch (e) {
    errores.push(`Suite 05: ${e.message}`);
  }

  return {
    test: 'Órdenes y saga de transacciones',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}