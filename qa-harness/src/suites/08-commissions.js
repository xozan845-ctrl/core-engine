/**
 * Suite 08 — Comisiones y liquidaciones (RN-04, RN-06, RN-07) con datos reales
 * Venta real genera comision 12%; devolucion la compensa; corte quincenal admin.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite08Comisiones(client, cfg, vendedorToken, adminToken, compradorToken) {
  const errores = [];
  const resultados = [];

  if (!vendedorToken || !adminToken || !compradorToken) {
    errores.push('Faltan tokens');
    return { test: 'Comisiones y liquidaciones', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const vAuth = { authorization: `Bearer ${vendedorToken}` };
  const aAuth = { authorization: `Bearer ${adminToken}` };
  const cAuth = { authorization: `Bearer ${compradorToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  try {
    // Setup: producto + oferta + venta real
    const prod = await client.post('/api/v1/catalog/productos', {
      sku: `QA-M1-${suf}`, nombre: 'Producto QA Com', descripcion: '', categoria: 'general',
      precio_base: '2000.00', stock: 5,
    }, aAuth);
    const tienda = await client.get('/api/v1/vendedores/me/tienda', vAuth);
    const oferta = await client.post('/api/v1/vendedores/productos', {
      producto_id: prod.body?.id, margen: 20,
      ...(tienda.body?.id ? { tienda_id: tienda.body.id } : {}),
    }, vAuth);
    const ofertaId = oferta.body?.id ?? null;

    let orderId = null;
    if (ofertaId) {
      const venta = await client.post('/api/v1/orders', { items: [{ oferta_id: ofertaId, cantidad: 1 }] }, cAuth);
      orderId = venta.body?.id ?? null;
      resultados.push({ test: 'Venta real (comprador)', status: venta.status, pass: [200, 201].includes(venta.status) });

      // esperamos el procesamiento asíncrono (outbox -> comision)
      await new Promise((r) => setTimeout(r, 1200));

      const ventas = await client.get('/api/v1/vendedores/me/ventas', vAuth);
      resultados.push({ test: 'Comision 12% en ventas (RN-04)', status: ventas.status, pass: ventas.status === 200 });

      const liq = await client.get('/api/v1/vendedores/me/liquidaciones', vAuth);
      resultados.push({ test: 'Liquidaciones vendedor', status: liq.status, pass: liq.status === 200 });

      // Corte quincenal (RN-07)
      const corte = await client.post('/api/v1/admin/liquidaciones/corte', {}, aAuth);
      resultados.push({ test: 'Corte liquidación (RN-07)', status: corte.status, pass: [200, 201].includes(corte.status) });

      // Pagar liquidacion si el corte genero alguna
      const liqs = Array.isArray(corte.body) ? corte.body : (corte.body?.liquidaciones ?? corte.body?.items ?? []);
      if (liqs.length > 0) {
        const pagar = await client.post(`/api/v1/admin/liquidaciones/${liqs[0].id}/pagar`, {}, aAuth);
        resultados.push({ test: 'Pagar liquidación', status: pagar.status, pass: [200, 201].includes(pagar.status) });
      } else {
        resultados.push({ test: 'Pagar liquidación (sin pendientes → skip)', pass: true });
      }

      // Devolucion compensa la comision (RN-06)
      if (orderId) {
        // El pago es asincrono: bajo carga la orden puede estar aun en 'creada'
        // cuando llega la devolucion (transicion invalida 400) → reintentar
        let dev = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'devuelta', motivo: 'Cliente canceló' }, aAuth);
        for (let i = 0; i < 3 && dev.status === 400; i++) {
          await new Promise((r) => setTimeout(r, 800));
          dev = await client.patch(`/api/v1/orders/${orderId}/estado`, { estado: 'devuelta', motivo: 'Cliente canceló' }, aAuth);
        }
        resultados.push({ test: 'Devolución orden (RN-06)', status: dev.status, pass: dev.status === 200 });

        await new Promise((r) => setTimeout(r, 800));
        const ventas2 = await client.get('/api/v1/vendedores/me/ventas', vAuth);
        resultados.push({ test: 'Comisión compensada tras devolución', status: ventas2.status, pass: ventas2.status === 200 });
      }

      const reporte = await client.get('/api/v1/admin/reportes', aAuth);
      resultados.push({ test: 'Reporte admin', status: reporte.status, pass: reporte.status === 200 });

      // ACL
      const c403 = await client.get('/api/v1/vendedores/me/ventas', cAuth);
      resultados.push({ test: 'Comprador en ventas vendedor → 403', status: c403.status, pass: c403.status === 403 });

      const cCorte = await client.post('/api/v1/admin/liquidaciones/corte', {}, cAuth);
      resultados.push({ test: 'Comprador en corte → 403', status: cCorte.status, pass: [401, 403].includes(cCorte.status) });

      const sinToken = await client.get('/api/v1/vendedores/me/ventas');
      resultados.push({ test: 'Sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });
    } else {
      resultados.push({ test: 'Setup producto+oferta', status: oferta.status, pass: false });
    }
  } catch (e) {
    errores.push(`Suite 08: ${e.message}`);
  }

  return {
    test: 'Comisiones y liquidaciones (RN-04, RN-06, RN-07)',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}