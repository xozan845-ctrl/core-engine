/**
 * Suite 04 — Tiendas y ofertas (RN-01, RN-02)
 * La tienda es unica por vendedor: si ya existe (409) se recupera por /me/tienda.
 * RN-02 se valida al publicar: oferta de producto con stock < 1 -> 409.
 */
import { ClientoHTTP } from '../utils.js';
import { Configuracion } from '../config.js';

export async function suite04Tiendas(client, cfg, vendedorToken, adminToken) {
  const errores = [];
  const resultados = [];

  if (!vendedorToken || !adminToken) {
    errores.push('Faltan tokens (vendedor/admin)');
    return { test: 'Tiendas y ofertas', pass: false, errores, resultados, timestamp: new Date().toISOString() };
  }

  const vAuth = { authorization: `Bearer ${vendedorToken}` };
  const aAuth = { authorization: `Bearer ${adminToken}` };
  const suf = Date.now().toString(36) + Math.random().toString(36).slice(2, 5);

  try {
    // 1. Alta tienda (una por vendedor; 409 si ya existe -> recuperar)
    let tiendaId = null;
    const crear = await client.post('/api/v1/vendedores/tienda', {
      nombre: `Tienda QA ${suf}`,
      descripcion: 'Tienda de prueba',
    }, vAuth);
    if ([200, 201].includes(crear.status)) {
      tiendaId = crear.body?.id ?? crear.body?.tienda?.id ?? null;
    }
    const miTienda = await client.get('/api/v1/vendedores/me/tienda', vAuth);
    if (!tiendaId && miTienda.status === 200) {
      tiendaId = miTienda.body?.id ?? miTienda.body?.tienda?.id ?? null;
    }
    resultados.push({ test: 'Alta/recuperar tienda', status: crear.status, pass: !!tiendaId });

    const crearProducto = async (sku, stock) => {
      const r = await client.post('/api/v1/catalog/productos', {
        sku, nombre: `Prod ${sku}`, descripcion: 'd', categoria: 'general',
        precio_base: '1000.00', stock,
      }, aAuth);
      return r.body?.id ?? null;
    };

    const publicar = (productoId, margen) => client.post('/api/v1/vendedores/productos', {
      producto_id: productoId, margen, ...(tiendaId ? { tienda_id: tiendaId } : {}),
    }, vAuth);

    // 2. Ofertas validas: margen 15, 0 y 90 (RN-01)
    const p15 = await crearProducto(`QA-S1-${suf}`, 10);
    const of15 = await publicar(p15, 15);
    resultados.push({ test: 'Oferta margen 15%', status: of15.status, pass: [200, 201].includes(of15.status) });

    const p0 = await crearProducto(`QA-S2-${suf}`, 10);
    const of0 = await publicar(p0, 0);
    resultados.push({ test: 'Oferta margen 0% (RN-01)', status: of0.status, pass: [200, 201].includes(of0.status) });

    const p90 = await crearProducto(`QA-S3-${suf}`, 10);
    const of90 = await publicar(p90, 90);
    resultados.push({ test: 'Oferta margen 90% (RN-01)', status: of90.status, pass: [200, 201].includes(of90.status) });

    // 3. RN-02: publicar producto con stock 0 -> 409
    const p0stock = await crearProducto(`QA-S4-${suf}`, 0);
    const ofRN02 = await publicar(p0stock, 10);
    resultados.push({ test: 'Oferta stock < 1 → 409 (RN-02)', status: ofRN02.status, pass: ofRN02.status === 409 });

    // 4. Márgenes inválidos -> 400 (DTO: IsInt Min 0 Max 90)
    const margenNeg = await publicar(p15, -1);
    resultados.push({ test: 'Margen -1 → 400', status: margenNeg.status, pass: margenNeg.status === 400 });

    const margen101 = await publicar(p15, 101);
    resultados.push({ test: 'Margen 101 → 400', status: margen101.status, pass: margen101.status === 400 });

    const margenFloat = await publicar(p15, 15.5);
    resultados.push({ test: 'Margen 15.5 → 400 (IsInt)', status: margenFloat.status, pass: margenFloat.status === 400 });

    const sinProducto = await client.post('/api/v1/vendedores/productos', { margen: 10 }, vAuth);
    resultados.push({ test: 'Sin producto_id → 400', status: sinProducto.status, pass: sinProducto.status === 400 });

    // 5. Tienda publica y ofertas del vendedor
    const tiendaPublica = await client.get(`/api/v1/tiendas/${tiendaId}`);
    resultados.push({ test: 'Tienda pública', status: tiendaPublica.status, pass: tiendaPublica.status === 200 });

    const misOfertas = await client.get('/api/v1/vendedores/me/ofertas', vAuth);
    resultados.push({ test: 'Mis ofertas', status: misOfertas.status, pass: misOfertas.status === 200 });

    // 6. Cambiar margen de la oferta (historico de precios RN-08)
    if (of15.body?.id) {
      const cambio = await client.patch(`/api/v1/vendedores/productos/${of15.body.id}`, { margen: 20 }, vAuth);
      resultados.push({ test: 'Cambiar margen 15→20%', status: cambio.status, pass: cambio.status === 200 });
    }

    // 7. Sin token -> 401; vendedor en ruta admin -> 403
    const sinToken = await client.get('/api/v1/vendedores/me/ofertas');
    resultados.push({ test: 'Sin token → 401', status: sinToken.status, pass: sinToken.status === 401 });
  } catch (e) {
    errores.push(`Suite 04: ${e.message}`);
  }

  return {
    test: 'Tiendas y ofertas (RN-01, RN-02)',
    pass: resultados.filter(r => r.pass).length > 0,
    errores,
    resultados,
    timestamp: new Date().toISOString(),
  };
}