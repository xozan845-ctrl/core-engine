import { EVENTOS, CONTRATOS } from './contracts';
import type { NombreEvento } from './contracts';

describe('contracts (eventos del bus, Tabla 22)', () => {
  it('debe definir nombres de evento unicos sin duplicados', () => {
    const nombres = Object.values(EVENTOS);
    expect(new Set(nombres).size).toBe(nombres.length);
  });

  it('debe mapear cada evento a un contrato con editor y consumidores', () => {
    for (const [clave, nombre] of Object.entries(EVENTOS)) {
      const contrato = CONTRATOS[nombre as NombreEvento];
      expect(contrato).toBeDefined();
      expect(contrato.editor.length).toBeGreaterThan(0);
      expect(Array.isArray(contrato.consumidores)).toBe(true);
      expect(contrato.consumidores.length).toBeGreaterThan(0);
      expect(clave).toBeTruthy();
    }
  });

  it('debe nombrar los eventos criticos del ciclo de la orden con el contrato exacto', () => {
    expect(EVENTOS.ORDER_CREATED).toBe('order.created');
    expect(EVENTOS.STOCK_RESERVADO).toBe('stock.reservado');
    expect(EVENTOS.STOCK_FALLIDO).toBe('stock.fallido');
    expect(EVENTOS.PAYMENT_PROCESADO).toBe('payment.procesado');
    expect(EVENTOS.ORDER_COMPLETADO).toBe('order.completado');
    expect(EVENTOS.COMISION_ACREDITADA).toBe('comision.acreditada');
    expect(EVENTOS.DECLARACION_GENERADA).toBe('declaracion.generada');
  });

  it('debe relacionar la venta geolocalizada con inteligencia de mercado', () => {
    const contrato = CONTRATOS[EVENTOS.VENTA_GEOLOCALIZADA];
    expect(contrato.consumidores).toContain('Inteligencia de Mercado');
    expect(contrato.editor).toContain('Field');
  });

  it('debe vaciar los consumidores de las notificaciones en la saga de inventario', () => {
    expect(CONTRATOS[EVENTOS.STOCK_RESERVADO].consumidores).toContain('Pedidos (confirmacion)');
    expect(CONTRATOS[EVENTOS.STOCK_UPDATED].consumidores).toContain('Notificaciones');
  });
});