import { ContabilidadService } from './contabilidad.service';
import type { PoolClient } from 'pg';

/**
 * Libros DGI (cap. 4.4) y asiento de IVA trasladado al emitir comprobantes
 * (Ley 822, 15 %): el pasivo fiscal (2.1.3) se reconoce contra fondos por
 * liquidar (2.1.2) dentro de la misma transaccion del comprobante.
 */
describe('Contabilidad y facturacion (libros DGI e IVA)', () => {
  describe('libro de ventas y libro de compras (cap. 4.4)', () => {
    it('libroVentas consolida por mes los comprobantes EMITIDOS', async () => {
      const filas = [
        { periodo: '2026-08', comprobantes: 2, base_gravada_cents: 90000, iva_cents: 13500, exento_cents: 0, total_cents: 103500 },
      ];
      const pg = { query: jest.fn().mockResolvedValue(filas) };
      const servicio = new ContabilidadService(pg as never, { insertarEnTransaccion: jest.fn() } as never);
      const resultado = await servicio.libroVentas('2026-08-01', '2026-08-31');
      expect(resultado).toEqual(filas);
      const sql = (pg.query as jest.Mock).mock.calls[0][0] as string;
      expect(sql).toContain("estado = 'EMITIDO'");
      expect(sql).toContain('GROUP BY TO_CHAR(emitido_en');
      expect(sql).toContain('SUM(iva_cents)');
    });

    it('libroCompras refleja el MVP sin compras: credito fiscal en cero', async () => {
      const pg = { query: jest.fn().mockResolvedValue([{ periodo: '2026-08', comprobantes: 0, base_gravada_cents: 0, iva_cents: 0, exento_cents: 0, total_cents: 0 }]) };
      const servicio = new ContabilidadService(pg as never, { insertarEnTransaccion: jest.fn() } as never);
      const resultado = await servicio.libroCompras();
      expect(resultado[0].iva_cents).toBe(0);
      expect(resultado[0].comprobantes).toBe(0);
    });
  });

  describe('asiento de IVA al emitir comprobante (Ley 822)', () => {
    const crearFacturacion = (emitir: boolean, contabilidad: ContabilidadService) => {
      const prev = process.env.EMITIR_COMPROBANTES_FISCALES;
      process.env.EMITIR_COMPROBANTES_FISCALES = String(emitir);
      // eslint-disable-next-line @typescript-eslint/no-var-requires
      const { FacturacionService } = require('../facturacion/facturacion.service') as typeof import('../facturacion/facturacion.service');
      const pg = {
        query: jest.fn().mockResolvedValue([]),
        queryOne: jest.fn(),
        transaccion: jest.fn(async (fn: (c: PoolClient) => Promise<unknown>) => fn(clientActual)),
      };
      const outbox = { insertarEnTransaccion: jest.fn().mockResolvedValue(undefined) };
      const cont = { registrarEnTransaccion: jest.fn().mockResolvedValue({ id: 'asiento-1' }) } as unknown as ContabilidadService;
      const servicio = new FacturacionService(pg as never, outbox as never, cont);
      if (prev === undefined) delete process.env.EMITIR_COMPROBANTES_FISCALES;
      else process.env.EMITIR_COMPROBANTES_FISCALES = prev;
      return { servicio, pg, outbox, cont };
    };

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let clientActual: any;

    const comprobante = {
      id: 'c1',
      serie: 'F001',
      numero: 'F00000001',
      tipo: 'FACTURA',
      orden_id: 'o1',
      cliente_id: null,
      base_gravada_cents: 45000,
      iva_cents: 6750, // 15 %
      exento_cents: 0,
      total_cents: 51750,
      moneda: 'C$',
      estado: 'EMITIDO',
      datos_cliente: null,
      emitido_en: '2026-08-01T00:00:00Z',
      creado_en: '2026-08-01T00:00:00Z',
    };

    const mockFetch = () => {
      jest.spyOn(globalThis, 'fetch').mockResolvedValue({
        ok: true,
        json: async () => ({ total_cents: 45000 }),
      } as unknown as Response);
    };

    it('EMITIDO: registra el pasivo 2.1.2 -> 2.1.3 por el IVA trasladado', async () => {
      clientActual = {
        query: jest.fn()
          .mockResolvedValueOnce({ rows: [comprobante] }) // INSERT comprobante
          .mockResolvedValueOnce({ rows: [] }), // UPDATE serie
      };
      const { servicio, cont } = crearFacturacion(true, null as never);
      mockFetch();

      const pgSerie = (servicio as unknown as { pg: { queryOne: jest.Mock } }).pg;
      pgSerie.queryOne
        .mockResolvedValueOnce({ serie: 'F001', tipo: 'FACTURA', prefijo: 'F', secuencial_actual: 0, ruc_emisor: 'RUC', jurisdiccion: 'NI', activa: true })
        .mockResolvedValueOnce({ tasa_iva: 0.15 });

      const resultado = await servicio.emitir('FACTURA', 'o1');
      expect(resultado.estado).toBe('EMITIDO');
      expect(resultado.iva_cents).toBe(6750);
      expect(cont.registrarEnTransaccion).toHaveBeenCalledTimes(1);
      const params = (cont.registrarEnTransaccion as jest.Mock).mock.calls[0][1];
      expect(params.detalles).toEqual(
        expect.arrayContaining([
          expect.objectContaining({ cuenta_codigo: '2.1.2', debe_cents: 6750 }),
          expect.objectContaining({ cuenta_codigo: '2.1.3', haber_cents: 6750 }),
        ]),
      );
      const debe = params.detalles.reduce((s: number, d: { debe_cents: number }) => s + d.debe_cents, 0);
      const haber = params.detalles.reduce((s: number, d: { haber_cents: number }) => s + d.haber_cents, 0);
      expect(debe).toBe(haber); // partida doble
    });

    it('BORRADOR: no registra asiento mientras la facturacion fiscal este apagada', async () => {
      clientActual = {
        query: jest.fn()
          .mockResolvedValueOnce({ rows: [{ ...comprobante, estado: 'BORRADOR', emitido_en: null }] })
          .mockResolvedValueOnce({ rows: [] }),
      };
      const { servicio, cont } = crearFacturacion(false, null as never);
      mockFetch();

      const pgSerie = (servicio as unknown as { pg: { queryOne: jest.Mock } }).pg;
      pgSerie.queryOne
        .mockResolvedValueOnce({ serie: 'F001', tipo: 'FACTURA', prefijo: 'F', secuencial_actual: 1, ruc_emisor: 'RUC', jurisdiccion: 'NI', activa: true })
        .mockResolvedValueOnce({ tasa_iva: 0.15 });

      const resultado = await servicio.emitir('FACTURA', 'o1');
      expect(resultado.estado).toBe('BORRADOR');
      expect(cont.registrarEnTransaccion).not.toHaveBeenCalled();
    });
  });
});