import test from 'node:test';
import assert from 'node:assert/strict';
import {
  clasificarRuta,
  construirDocumento,
  construirDocumentos,
} from './swagger-aggregate.mjs';

const operacion = (summary) => ({
  summary,
  responses: { 200: { description: '' } },
});

const spec = (fichero, paths, schemas = {}) => ({
  fichero,
  spec: {
    paths,
    components: {
      schemas,
      securitySchemes: {
        bearer: { type: 'http', scheme: 'bearer', bearerFormat: 'JWT' },
      },
    },
  },
});

test('clasifica rutas por superficie y rechaza prefijos desconocidos', () => {
  assert.equal(clasificarRuta('/api/v1/catalog/productos'), 'publica');
  assert.equal(clasificarRuta('/internal/productos/lote'), 'interna');
  assert.throws(() => clasificarRuta('/health'), /superficies conocidas/);
  assert.throws(() => clasificarRuta('/api/v2/catalog/productos'), /superficies conocidas/);
});

test('el contrato público excluye /internal y el interno excluye /api/v1', () => {
  const { publica, interna } = construirDocumentos([
    spec('catalog-service.json', {
      '/api/v1/catalog/productos': { get: operacion('productos') },
      '/internal/productos/lote': { get: operacion('lote') },
    }),
    spec('identity-service.json', {
      '/internal/usuarios/{id}': { get: operacion('usuario') },
    }),
  ]);

  assert.deepEqual(Object.keys(publica.paths), ['/api/v1/catalog/productos']);
  assert.deepEqual(Object.keys(interna.paths), [
    '/internal/productos/lote',
    '/internal/usuarios/{id}',
  ]);
});

test('el contrato interno declara x-internal-key y el público conserva bearer', () => {
  const { publica, interna } = construirDocumentos([
    spec('catalog-service.json', {
      '/api/v1/catalog/productos': { get: operacion('productos') },
      '/internal/productos/lote': { get: operacion('lote') },
    }),
  ]);

  assert.deepEqual(publica.components.securitySchemes.bearer, {
    type: 'http',
    scheme: 'bearer',
    bearerFormat: 'JWT',
  });
  assert.equal(publica.security, undefined);
  assert.deepEqual(interna.components.securitySchemes.internalKey, {
    type: 'apiKey',
    in: 'header',
    name: 'x-internal-key',
  });
  assert.deepEqual(interna.security, [{ internalKey: [] }]);
});

test('falla si dos specs documentan el mismo path y método en la misma superficie', () => {
  const especificaciones = [
    spec('catalog-service.json', {
      '/api/v1/catalog/productos': { get: operacion('uno') },
    }),
    spec('stores-service.json', {
      '/api/v1/catalog/productos': { get: operacion('dos') },
    }),
  ];

  assert.throws(
    () => construirDocumento(especificaciones, 'publica'),
    /colisiones detectadas/,
  );
});

test('no incluye schemas de specs que no aportan rutas a la superficie', () => {
  const especificaciones = [
    spec(
      'identity-service.json',
      { '/internal/usuarios/{id}': { get: operacion('usuario') } },
      { UsuarioInterno: { type: 'object' } },
    ),
    spec(
      'catalog-service.json',
      { '/api/v1/catalog/productos': { get: operacion('productos') } },
      { Producto: { type: 'object' } },
    ),
  ];

  const { publica, interna } = construirDocumentos(especificaciones);

  assert.deepEqual(Object.keys(publica.components.schemas), ['Producto']);
  assert.deepEqual(Object.keys(interna.components.schemas), ['UsuarioInterno']);
});
