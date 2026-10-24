import { securityHeaders } from './security-headers.middleware';

const crearRes = () => {
  const res = {
    headers: {} as Record<string, string>,
    setHeader(clave: string, valor: string) {
      this.headers[clave] = valor;
    },
  };
  return res;
};

describe('securityHeaders (hardening del borde)', () => {
  it('debe emitir las cabeceras base y las de aislamiento del navegador', () => {
    const res = crearRes();
    const next = vi.fn();

    securityHeaders()({} as never, res as never, next);

    expect(next).toHaveBeenCalledTimes(1);
    expect(res.headers['X-Content-Type-Options']).toBe('nosniff');
    expect(res.headers['X-Frame-Options']).toBe('DENY');
    expect(res.headers['Referrer-Policy']).toBe('no-referrer');
    expect(res.headers['Content-Security-Policy']).toContain("default-src 'none'");
    expect(res.headers['Strict-Transport-Security']).toContain('max-age=31536000');
    expect(res.headers['Permissions-Policy']).toContain('camera=()');
    expect(res.headers['Cross-Origin-Opener-Policy']).toBe('same-origin');
    expect(res.headers['Cross-Origin-Embedder-Policy']).toBe('require-corp');
    expect(res.headers['Cross-Origin-Resource-Policy']).toBe('same-site');
    expect(res.headers['X-Permitted-Cross-Domain-Policies']).toBe('none');
  });
});
