import { ROLES } from '@core/shared';
import { ComercioController } from './comercio.controller';

const usuario = {
  user_id: 'u1',
  email: 'empresa@test.dev',
  rol: ROLES.EMPRESARIO,
  tenant_id: 't1',
};

describe('ComercioController', () => {
  it('delega todas las operaciones en el servicio', async () => {
    const llamadas: string[] = [];
    const servicio = new Proxy(
      {},
      {
        get: (_destino, propiedad) => {
          if (typeof propiedad !== 'string') return undefined;
          return (...args: unknown[]) => {
            llamadas.push(propiedad);
            return Promise.resolve({ args });
          };
        },
      },
    );
    const controlador = new ComercioController(servicio as never);
    const metodos = Object.getOwnPropertyNames(ComercioController.prototype).filter(
      (nombre) => nombre !== 'constructor',
    );

    expect(metodos.length).toBeGreaterThan(50);

    for (const metodo of metodos) {
      const ejecutar = (controlador as unknown as Record<string, (...args: unknown[]) => unknown>)[
        metodo
      ];
      const resultado = ejecutar.call(controlador, 'id-1', {}, usuario, '2026-01-01');
      await expect(Promise.resolve(resultado)).resolves.toBeDefined();
    }

    expect(llamadas).toContain('crearTransferencia');
    expect(llamadas).toContain('previewCierre');
    expect(llamadas).toContain('importarClientes');
    expect(llamadas).toContain('actualizarConfiguracionDocumentos');
    expect(new Set(llamadas).size).toBeGreaterThan(40);
    expect(llamadas.length).toBe(metodos.length);
  });
});
