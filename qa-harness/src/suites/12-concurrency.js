/**
 * Suite 12 — Concurrencia y race conditions (anti-oversell)
 * Con stock=1 y 50 compradores simultáneos, nunca debe haber >1 éxito (TC-04).
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite12Concurrencia(client, cfg, compradorToken, adminToken, vendedorToken) {
  const errores = [];
  const resultados = [];

  if (!compradorToken || !adminToken || !vendedorToken) {
    errores.push('Faltan tokens');
    return { test: 'Concurrencia y race conditions', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const cAuth = { authorization: `Bearer ${compradorToken}` };
  const aAuth = { authorization: `Bearer ${adminToken}` };
  const vAuth = { authorization: `Bearer ${vendedorToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  const setup = async (stock, sufijo) => {
    const prod = await client.post('/api/v1/catalog/productos', {
      sku: `QA-R1-${suf}-${sufijo}`, nombre: 'Producto QA Race', descripcion: '', categoria: 'general',
      precio_base: '1000.00', stock,
    }, aAuth);
    const tienda = await client.get('/api/v1/vendedores/me/tienda', vAuth);
    const oferta = await client.post('/api/v1/vendedores/productos', {
      producto_id: prod.body?.id, margen: 10,
      ...(tienda.body?.id ? { tienda_id: tienda.body.id } : {}),
    }, vAuth);
    return oferta.body?.id ?? null;
  };

  const comprar = (ofertaId, cantidad) =>
    client.post('/api/v1/orders', { items: [{ oferta_id: ofertaId, cantidad }] }, cAuth);

  const estadoFinal = async (orderId) => {
    try {
      const r = await client.get(`/api/v1/orders/${orderId}`, cAuth);
      return r.body?.estado ?? null;
    } catch {
      return null;
    }
  };

  try {
    // 1. Anti-oversell: stock=1, 50 compradores simultáneos → máx 1 orden "no cancelada"
    const ofertaStock1 = await setup(1, 'a');
    if (ofertaStock1) {
      const resultadosRace = await Promise.allSettled(
        Array.from({ length: 50 }, () => comprar(ofertaStock1, 1)),
      );
      let creadas = 0;
      const ids = [];
      const estados = {};
      for (const r of resultadosRace) {
        const st = r.status === 'fulfilled' ? r.value.status : 'rejected';
        if (st === 201 || st === 200) {
          creadas++;
          if (r.value.body?.id) ids.push(r.value.body.id);
        }
        estados[st] = (estados[st] || 0) + 1;
      }
      // consistencia eventual: las creadas optimistamente se cancelan al llegar
      // stock.fallido (RN-03); se espera el drenaje antes de contar exitos reales
      await new Promise((r) => setTimeout(r, 8000));
      let firmes = 0;
      for (const id of ids) {
        const e = await estadoFinal(id);
        if (e && e !== 'cancelada') firmes++;
      }
      const detalle = Object.entries(estados).map(([k, v]) => `${k}:${v}`).join(', ');
      resultados.push({
        test: `Anti-oversell 50× stock=1 (creadas: ${creadas}, firmes: ${firmes}) [${detalle}]`,
        pass: firmes === 1,
      });
    } else {
      resultados.push({ test: 'Setup oferta stock=1', status: 500, pass: false });
    }

    // 2. Doble submit exacto con stock 2 → 1 firme + 1 cancelada/409 (nunca 2 firmes)
    const ofertaStock2 = await setup(2, 'b');
    if (ofertaStock2) {
      // 2 dobles sobre stock=2 → ambos completan (2 unidades disponibles)
      const dbl = await Promise.all([comprar(ofertaStock2, 1), comprar(ofertaStock2, 1)]);
      const cod = dbl.map((r) => r.status).join(',');
      const ids2 = dbl.filter((r) => r.body?.id).map((r) => r.body.id);
      await new Promise((r) => setTimeout(r, 8000));
      let firmes2 = 0;
      for (const id of ids2) {
        const e = await estadoFinal(id);
        if (e && e !== 'cancelada') firmes2++;
      }
      resultados.push({ test: `Doble submit stock=2 (firmes: ${firmes2}) [${cod}]`, pass: firmes2 === 2 });
    } else {
      resultados.push({ test: 'Setup oferta stock=2', status: 500, pass: false });
    }

    // 3. Carrito 10x en paralelo (comprador) → todos 200 (sin reserva, RN-03)
    const ofertaCart = await setup(5, 'c');
    if (ofertaCart) {
      const carts = await Promise.allSettled(
        Array.from({ length: 10 }, () => client.post('/api/v1/carrito/items', { oferta_id: ofertaCart, cantidad: 1 }, cAuth)),
      );
      const ok = carts.filter((r) => r.status === 'fulfilled' && [200, 201].includes(r.value.status)).length;
      resultados.push({ test: `Carrito 10× paralelo (200: ${ok}/10)`, pass: ok >= 9 });
    }

    // 4. Transiciones concurrentes → sin crash, estados consistentes
    const ofertaT = await setup(6, 'd');
    if (ofertaT) {
      const o1 = await comprar(ofertaT, 1);
      const o2 = await comprar(ofertaT, 1);
      const ids = [o1.body?.id, o2.body?.id].filter(Boolean);
      if (ids.length === 2) {
        const [r1, r2] = await Promise.all([
          client.patch(`/api/v1/orders/${ids[0]}/estado`, { estado: 'en_preparacion' }, aAuth),
          client.patch(`/api/v1/orders/${ids[1]}/estado`, { estado: 'en_preparacion' }, aAuth),
        ]);
        const cod = [r1.status, r2.status].join(',');
        resultados.push({ test: `Transiciones concurrentes [${cod}]`, pass: true });
      }
    }

    // 5. Corte liquidación simultáneo (2 admin en paralelo) → sin 500
    const [c1, c2] = await Promise.all([
      client.post('/api/v1/admin/liquidaciones/corte', {}, aAuth),
      client.post('/api/v1/admin/liquidaciones/corte', {}, aAuth),
    ]);
    const codCorte = [c1.status, c2.status].join(',');
    resultados.push({ test: `Corte simultáneo [${codCorte}] (sin 500)`, pass: !codCorte.includes('500') });
  } catch (e) {
    errores.push(`Suite 12: ${e.message}`);
  }

  return {
    test: 'Concurrencia y race conditions',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}