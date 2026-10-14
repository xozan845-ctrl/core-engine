/**
 * Contrato de una suite de reglas. Cada suite declara su identidad y registra
 * sus `test()` de `node:test`.
 *
 * El registro se puede desactivar (`__setRegistration(false)`) para que el
 * runner lea la metadata **sin** ejecutar los tests: es la pieza que permite
 * descubrir suites importándolas sin ensuciar el proceso padre.
 */
import test, { describe } from 'node:test';
import assert from 'node:assert/strict';

const MODES = new Set(['pure', 'repo']);

let registrationEnabled = true;

/** Activa/desactiva el registro de tests (uso interno del runner). */
export function __setRegistration(enabled) {
  registrationEnabled = enabled;
}

/**
 * Declara una suite de reglas.
 * @param {object} spec
 * @param {string} spec.id Identificador único de la suite.
 * @param {string} [spec.area] Área de `docs/rules` que vigila.
 * @param {string[]} [spec.rules] IDs de regla que la suite verifica.
 * @param {'pure'|'repo'} [spec.mode] `pure` no toca disco/git; `repo` inspecciona el repo.
 * @param {(ctx: { test: typeof test, assert: typeof assert }) => void} spec.run
 *   Registra los `test()` de la suite.
 * @returns {Readonly<{ id: string, area?: string, rules: string[], mode: string }>}
 */
export function defineSuite(spec) {
  const { id, area, rules = [], mode = 'repo', run } = spec ?? {};

  if (!id) throw new Error('defineSuite: falta el campo "id"');
  if (typeof run !== 'function') throw new Error(`defineSuite(${id}): falta "run"`);
  if (!MODES.has(mode)) {
    throw new Error(`defineSuite(${id}): "mode" debe ser pure|repo (recibido: ${mode})`);
  }

  if (registrationEnabled) {
    describe(id, () => run({ test, assert }));
  }

  return Object.freeze({ id, area, rules: [...rules], mode });
}
