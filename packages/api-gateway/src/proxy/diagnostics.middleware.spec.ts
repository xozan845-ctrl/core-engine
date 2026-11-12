import { describe, expect, it, vi } from 'vitest';
import { diagnostico, leerDiagnostico } from './diagnostics.middleware';

function crearReq(ruta: string, headers: Record<string, string> = {}) {
  return { path: ruta, method: 'GET', headers } as never;
}

function crearRes() {
  const res = {
    status: vi.fn().mockReturnThis(),
    json: vi.fn().mockReturnThis(),
    setHeader: vi.fn().mockReturnThis(),
  };
  return res;
}

function ejecutar(ruta: string, headers: Record<string, string> = {}, opciones = {}) {
  const req = crearReq(ruta, headers);
  const res = crearRes();
  const next = vi.fn();
  diagnostico({ diagnosticosPublicos: false, readyPublico: false, ...opciones })(
    req,
    res as never,
    next,
  );
  return { res, next };
}

describe('diagnostico (ADR-0005/R-GW-8)', () => {
  it('debe dejar /health siempre abierto', () => {
    const { res, next } = ejecutar('/health');
    expect(next).toHaveBeenCalledOnce();
    expect(res.status).not.toHaveBeenCalled();
  });

  it('debe ocultar /metrics y /docs con 404 sin clave interna', () => {
    for (const ruta of ['/metrics', '/docs', '/docs-json', '/docs-yaml']) {
      const { res, next } = ejecutar(ruta, {}, { claveInterna: 'interna' });
      expect(res.status).toHaveBeenCalledWith(404);
      expect(next).not.toHaveBeenCalled();
    }
  });

  it('debe abrir el diagnóstico con la clave interna correcta', () => {
    const { res, next } = ejecutar(
      '/metrics',
      { 'x-internal-key': 'interna' },
      { claveInterna: 'interna' },
    );
    expect(next).toHaveBeenCalledOnce();
    expect(res.status).not.toHaveBeenCalled();
  });

  it('debe abrir el diagnóstico cuando el entorno lo declara público', () => {
    const { next } = ejecutar('/docs', {}, { diagnosticosPublicos: true });
    expect(next).toHaveBeenCalledOnce();
  });

  it('debe ocultar /ready sin clave salvo READY_PUBLIC', () => {
    const oculto = ejecutar('/ready', {}, { claveInterna: 'interna' });
    expect(oculto.res.status).toHaveBeenCalledWith(404);

    const publico = ejecutar('/ready', {}, { readyPublico: true });
    expect(publico.next).toHaveBeenCalledOnce();

    const conClave = ejecutar(
      '/ready',
      { 'x-internal-key': 'interna' },
      { claveInterna: 'interna' },
    );
    expect(conClave.next).toHaveBeenCalledOnce();
  });

  it('debe dejar pasar el resto de rutas', () => {
    const { next } = ejecutar('/api/v2/auth/login', {}, { claveInterna: 'interna' });
    expect(next).toHaveBeenCalledOnce();
  });

  it('debe leer la configuración con defaults seguros', () => {
    expect(leerDiagnostico({})).toEqual({
      diagnosticosPublicos: false,
      readyPublico: false,
      claveInterna: undefined,
    });
    expect(
      leerDiagnostico({
        DIAGNOSTICOS_PUBLICOS: 'true',
        READY_PUBLIC: 'true',
        INTERNAL_API_KEY: 'clave',
      }),
    ).toEqual({
      diagnosticosPublicos: true,
      readyPublico: true,
      claveInterna: 'clave',
    });
  });
});
