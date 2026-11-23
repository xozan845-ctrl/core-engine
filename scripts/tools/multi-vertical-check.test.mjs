import assert from 'node:assert/strict';
import test from 'node:test';
import { validarCierre } from './multi-vertical-check.mjs';

const base = {
  empresas: [
    { id: 'e-1', pack_codigo: 'pack_retail', pack_version: 1, region_tributaria: 'NI' },
  ],
  migraciones: [{ empresa_id: 'e-1', estado: 'aplicado' }],
  packs: [{ codigo: 'pack_retail', version: 1, activo: true }],
  capacidades: [{ codigo: 'catalogo_items' }],
  packCapacidades: [
    { pack_codigo: 'pack_retail', pack_version: 1, capacidad_codigo: 'catalogo_items' },
  ],
  empresaCapacidades: [{ empresa_id: 'e-1', capacidad_codigo: 'catalogo_items' }],
  packRegiones: [
    {
      pack_codigo: 'pack_retail',
      pack_version: 1,
      region_codigo: 'NI',
      predeterminada: true,
      activo: true,
    },
  ],
  jurisdicciones: [{ codigo_pais: 'NI', activa: true }],
};

test('debe aprobar el cierre cuando no hay inconsistencias de packs ni regiones', () => {
  assert.deepEqual(validarCierre(base), {
    ok: true,
    problemas: [],
    resumen: {
      empresas: 1,
      packs_activos: 1,
      migraciones_pendientes: 0,
      regiones_activas: 1,
    },
  });
});

test('debe detectar tenants sin pack y migraciones pendientes', () => {
  const resultado = validarCierre({
    ...base,
    empresas: [
      { id: 'e-1', pack_codigo: null, pack_version: null, region_tributaria: 'NI' },
    ],
    migraciones: [{ empresa_id: 'e-1', estado: 'requiere_revision' }],
  });
  assert.equal(resultado.ok, false);
  assert.deepEqual(resultado.problemas, [
    'empresa e-1: sin pack asignado',
    'empresa e-1: migracion requiere_revision',
  ]);
});

test('debe detectar capacidades de pack que no existen en el catalogo', () => {
  const resultado = validarCierre({
    ...base,
    packCapacidades: [
      { pack_codigo: 'pack_retail', pack_version: 1, capacidad_codigo: 'desconocida' },
    ],
  });
  assert.equal(resultado.ok, false);
  assert.deepEqual(resultado.problemas, [
    'pack pack_retail@1: capacidad desconocida desconocida',
  ]);
});

test('debe detectar empresas sin region o con una region inexistente', () => {
  const sinRegion = validarCierre({
    ...base,
    empresas: [
      { id: 'e-1', pack_codigo: 'pack_retail', pack_version: 1, region_tributaria: null },
    ],
  });
  assert.deepEqual(sinRegion.problemas, ['empresa e-1: sin region tributaria']);

  const inexistente = validarCierre({
    ...base,
    empresas: [
      { id: 'e-1', pack_codigo: 'pack_retail', pack_version: 1, region_tributaria: 'XX' },
    ],
  });
  assert.deepEqual(inexistente.problemas, [
    'empresa e-1: region tributaria inexistente XX',
  ]);
});

test('una region desactivada puede seguir asignada pero no sirve para packs nuevos', () => {
  const soloInactiva = validarCierre({
    ...base,
    jurisdicciones: [{ codigo_pais: 'NI', activa: false }],
  });
  // La empresa existente conserva NI; el pack se queda sin región activa.
  assert.deepEqual(soloInactiva.problemas, [
    'pack pack_retail@1: sin region tributaria activa',
  ]);
});

test('debe exigir una region predeterminada por pack activo', () => {
  const sinDefault = validarCierre({
    ...base,
    packRegiones: [
      {
        pack_codigo: 'pack_retail',
        pack_version: 1,
        region_codigo: 'NI',
        predeterminada: false,
        activo: true,
      },
    ],
  });
  assert.deepEqual(sinDefault.problemas, [
    'pack pack_retail@1: debe tener exactamente una region predeterminada',
  ]);

  const sinRegiones = validarCierre({ ...base, packRegiones: [] });
  assert.deepEqual(sinRegiones.problemas, [
    'pack pack_retail@1: sin region tributaria activa',
  ]);
});
