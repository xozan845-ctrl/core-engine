/**
 * Suite 03 — Catálogo unificado (RN-01, RN-02, RN-08)
 *
 * Productos, servicios y alquileres son items de `catalog.items`. RN-02 se
 * aplica al PUBLICAR la oferta (suite 04), no al crear el item: un producto con
 * stock 0 es un estado válido. Los códigos son únicos por empresa y tipo.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite03Catalogo(client, cfg, adminToken) {
  const errores = [];
  const resultados = [];

  if (!adminToken) {
    errores.push('No admin token provided');
    return { test: 'Catálogo unificado y reglas de negocio', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }
  const empresaId = cfg.get ? cfg.get('empresaId') : cfg.empresaId;
  if (!empresaId) {
    errores.push('Falta empresa_id (bootstrap del runner)');
    return { test: 'Catálogo unificado y reglas de negocio', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const auth = { authorization: `Bearer ${adminToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  try {
    const crea = async (overrides) => client.post('/api/v2/catalog/items', {
      tipo: 'producto',
      codigo: `QA-P-${suf}`,
      nombre: 'Producto QA',
      descripcion: 'desc',
      categoria: 'general',
      precio: '1000.00',
      track_inventory: true,
      stock: 10,
      empresa_id: empresaId,
      ...overrides,
    }, auth);

    const alta = await crea({});
    resultados.push({ test: 'Alta producto (TC-01)', status: alta.status, pass: [200, 201].includes(alta.status) });
    const itemId = alta.body?.item_id ?? alta.body?.id;

    // Stock 0 es válido: RN-02 se valida al publicar la oferta.
    const stock0 = await crea({ codigo: `QA-P0-${suf}`, stock: 0 });
    resultados.push({ test: 'Item stock 0 → 201 (RN-02 en oferta)', status: stock0.status, pass: [200, 201].includes(stock0.status) });

    const servicio = await crea({
      tipo: 'servicio',
      codigo: `QA-S-${suf}`,
      nombre: 'Servicio QA',
      precio: '500.00',
      track_inventory: false,
      duration_minutes: 30,
    });
    resultados.push({ test: 'Alta servicio en el mismo catálogo', status: servicio.status, pass: [200, 201].includes(servicio.status) });

    const alquiler = await crea({
      tipo: 'alquiler',
      codigo: `QA-A-${suf}`,
      nombre: 'Alquiler QA',
      precio: '800.00',
      track_inventory: false,
    });
    resultados.push({ test: 'Alta alquiler en el mismo catálogo', status: alquiler.status, pass: [200, 201].includes(alquiler.status) });

    const neg = await crea({ codigo: `QA-PN-${suf}`, precio: '-50.00' });
    resultados.push({ test: 'Precio negativo → 400', status: neg.status, pass: neg.status === 400 });

    const dup = await crea({});
    resultados.push({ test: 'Código duplicado → 409', status: dup.status, pass: dup.status === 409 });

    const nomVacio = await crea({ codigo: `QA-P2-${suf}`, nombre: '' });
    resultados.push({ test: 'Nombre vacío → 400', status: nomVacio.status, pass: nomVacio.status === 400 });

    const noNum = await crea({ codigo: `QA-P3-${suf}`, precio: 'abc' });
    resultados.push({ test: 'Precio no numérico → 400', status: noNum.status, pass: noNum.status === 400 });

    const stockNeg = await crea({ codigo: `QA-P4-${suf}`, stock: -1 });
    resultados.push({ test: 'Stock negativo → 400', status: stockNeg.status, pass: stockNeg.status === 400 });

    const lista = await client.get(`/api/v2/catalog/items?empresa_id=${encodeURIComponent(empresaId)}`, auth);
    resultados.push({ test: 'Listado de items de la empresa', status: lista.status, pass: lista.status === 200 && Array.isArray(lista.body?.items) });

    if (itemId) {
      const det = await client.get(`/api/v2/catalog/items/${itemId}?empresa_id=${encodeURIComponent(empresaId)}`, auth);
      resultados.push({ test: 'Detalle del item creado', status: det.status, pass: det.status === 200 && (det.body?.item_id ?? det.body?.id) === itemId });

      const det404 = await client.get(`/api/v2/catalog/items/00000000-0000-0000-0000-000000000000?empresa_id=${encodeURIComponent(empresaId)}`, auth);
      resultados.push({ test: 'Item inexistente → 404', status: det404.status, pass: det404.status === 404 });
    }

    const sinToken = await client.post('/api/v2/catalog/items', {
      tipo: 'producto', codigo: `QA-P5-${suf}`, nombre: 'x', precio: '1.00', stock: 1, empresa_id: empresaId,
    });
    resultados.push({ test: 'Alta sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });
  } catch (e) {
    errores.push(`Suite 03: ${e.message}`);
  }

  return {
    test: 'Catálogo unificado y reglas de negocio',
    pass: resultados.length > 0 && resultados.every((r) => r.pass),
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}
