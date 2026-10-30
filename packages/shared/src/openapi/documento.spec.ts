import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  clasificarRutaOpenApi,
  exportarDocumentoOpenApi,
  schemasAlcanzables,
  separarDocumentoOpenApi,
} from './documento';

const documento = {
  openapi: '3.0.0',
  info: { title: 'original', version: 'v1' },
  tags: [{ name: 'Catálogo' }],
  paths: {
    '/api/v1/catalog/productos': {
      get: {
        responses: {
          200: {
            content: { 'application/json': { schema: { $ref: '#/components/schemas/Producto' } } },
          },
        },
      },
    },
    '/internal/productos/lote': {
      get: {
        responses: {
          200: {
            content: { 'application/json': { schema: { $ref: '#/components/schemas/Lote' } } },
          },
        },
      },
    },
  },
  components: {
    schemas: {
      Producto: {
        type: 'object',
        properties: { empresa: { $ref: '#/components/schemas/Empresa' } },
      },
      Empresa: { type: 'object', properties: { id: { type: 'string' } } },
      Lote: { type: 'object', properties: { sku: { type: 'string' } } },
      SinUso: { type: 'object' },
    },
    securitySchemes: { bearer: { type: 'http', scheme: 'bearer' } },
  },
};

describe('documento OpenAPI por superficie', () => {
  it('clasifica rutas y rechaza prefijos desconocidos', () => {
    expect(clasificarRutaOpenApi('/api/v1/catalog/productos')).toBe('publica');
    expect(clasificarRutaOpenApi('/internal/productos')).toBe('interna');
    expect(() => clasificarRutaOpenApi('/health')).toThrow(/superficies conocidas/);
  });

  it('separa paths y calcula la clausura de schemas por superficie', () => {
    const { publico, interno } = separarDocumentoOpenApi(documento, 'catalog-service');

    expect(Object.keys(publico.paths as object)).toEqual(['/api/v1/catalog/productos']);
    expect(Object.keys(interno.paths as object)).toEqual(['/internal/productos/lote']);
    expect(publico['x-servicio']).toBe('catalog-service');
    expect(publico['x-superficie']).toBe('publica');
    expect(publico.info.title).toContain('API pública');
    expect(Object.keys(publico.components.schemas)).toEqual(['Empresa', 'Producto']);
    expect(Object.keys(interno.components.schemas)).toEqual(['Lote']);
    expect(interno.security).toEqual([{ internalKey: [] }]);
  });

  it('falla si una ruta o un $ref no se pueden resolver', () => {
    const roto = {
      ...documento,
      paths: { '/health': { get: {} } },
    };
    expect(() => separarDocumentoOpenApi(roto, 'x')).toThrow(/superficies conocidas/);

    const refRoto = {
      ...documento,
      paths: {
        '/api/v1/x': {
          get: { responses: { 200: { content: { 'application/json': { schema: { $ref: '#/components/schemas/NoExiste' } } } } } },
        },
      },
    };
    expect(() => separarDocumentoOpenApi(refRoto, 'x')).toThrow(/sin schema/);
  });

  it('schemasAlcanzables devuelve solo los referenciados', () => {
    const schemas = schemasAlcanzables(documento, documento.paths);
    expect(Object.keys(schemas).sort()).toEqual(['Empresa', 'Lote', 'Producto']);
  });

  it('exporta ambos archivos en la carpeta del servicio', () => {
    const destino = mkdtempSync(join(tmpdir(), 'openapi-split-'));
    exportarDocumentoOpenApi(documento, 'catalog-service', destino);

    const publico = JSON.parse(
      readFileSync(join(destino, 'catalog-service', 'public.openapi.json'), 'utf8'),
    );
    const interno = JSON.parse(
      readFileSync(join(destino, 'catalog-service', 'internal.openapi.json'), 'utf8'),
    );
    expect(publico['x-superficie']).toBe('publica');
    expect(interno['x-superficie']).toBe('interna');
  });
});
