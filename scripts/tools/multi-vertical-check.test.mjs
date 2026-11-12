import assert from 'node:assert/strict';
import test from 'node:test';
import { validarCierre } from './multi-vertical-check.mjs';

const base = {
  empresas: [{ id: 'e-1', pack_codigo: 'pack_retail', pack_version: 1 }],
  migraciones: [{ empresa_id: 'e-1', estado: 'aplicado' }],
  packs: [{ codigo: 'pack_retail', version: 1, activo: true }],
  capacidades: [{ codigo: 'catalogo_items' }],
  packCapacidades: [{ pack_codigo: 'pack_retail', pack_version: 1, capacidad_codigo: 'catalogo_items' }],
  empresaCapacidades: [{ empresa_id: 'e-1', capacidad_codigo: 'catalogo_items' }],
};

test('debe aprobar el cierre cuando no hay inconsistencias de packs', () => {
  assert.deepEqual(validarCierre(base), {
    ok: true,
    problemas: [],
    resumen: { empresas: 1, packs_activos: 1, migraciones_pendientes: 0 },
  });
});

test('debe detectar tenants sin pack y migraciones pendientes', () => {
  const resultado = validarCierre({
    ...base,
    empresas: [{ id: 'e-1', pack_codigo: null, pack_version: null }],
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
    packCapacidades: [{ pack_codigo: 'pack_retail', pack_version: 1, capacidad_codigo: 'desconocida' }],
  });
  assert.equal(resultado.ok, false);
  assert.deepEqual(resultado.problemas, [
    'pack pack_retail@1: capacidad desconocida desconocida',
  ]);
});
