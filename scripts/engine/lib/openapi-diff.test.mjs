import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareSpecs } from './openapi-diff.mjs';

function spec(paths, schemas = {}) {
  return { openapi: '3.0.0', paths, components: { schemas } };
}

const base = spec(
  {
    '/a': {
      get: {
        responses: {
          200: {
            content: {
              'application/json': {
                schema: { $ref: '#/components/schemas/A' },
              },
            },
          },
        },
      },
    },
  },
  {
    A: { type: 'object', properties: { id: { type: 'string' }, name: { type: 'string' } } },
  },
);

test('openapi-diff: los cambios aditivos no son breaking', () => {
  const next = JSON.parse(JSON.stringify(base));
  next.paths['/b'] = { get: { responses: { 200: { content: {} } } } };
  next.components.schemas.A.properties.extra = { type: 'string' };
  assert.deepEqual(compareSpecs(base, next), []);
});

test('openapi-diff: detecta path y operación eliminados', () => {
  const next = JSON.parse(JSON.stringify(base));
  delete next.paths['/a'];
  assert.ok(compareSpecs(base, next).includes('/a: path eliminado'));
});

test('openapi-diff: detecta propiedad de respuesta eliminada y cambio de tipo', () => {
  const next = JSON.parse(JSON.stringify(base));
  delete next.components.schemas.A.properties.name;
  next.components.schemas.A.properties.id.type = 'integer';
  const problems = compareSpecs(base, next);
  assert.ok(problems.some((p) => p.includes('response 200.name: propiedad eliminada')));
  assert.ok(problems.some((p) => p.includes('response 200.id: tipo')));
});

test('openapi-diff: detecta una propiedad de request nueva obligatoria', () => {
  const oldRequest = spec({
    '/a': {
      post: {
        requestBody: {
          content: {
            'application/json': { schema: { $ref: '#/components/schemas/Req' } },
          },
        },
        responses: { 201: { content: {} } },
      },
    },
  }, { Req: { type: 'object', properties: { a: { type: 'string' } } } });
  const newRequest = JSON.parse(JSON.stringify(oldRequest));
  newRequest.components.schemas.Req.required = ['b'];
  newRequest.components.schemas.Req.properties.b = { type: 'string' };
  const problems = compareSpecs(oldRequest, newRequest);
  assert.ok(problems.some((p) => p.includes('request.b: propiedad nueva obligatoria')));
});
