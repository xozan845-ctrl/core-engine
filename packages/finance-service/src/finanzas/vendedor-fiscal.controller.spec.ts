import 'reflect-metadata';
import { VendedorFiscalController } from './vendedor-fiscal.controller';
import type { TributacionService } from '../tributacion/tributacion.service';
import type { KpisService } from './kpis.service';
import { ROLES } from '@core/shared';

const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };

function fabricar(): {
  tributacion: Record<string, jest.Mock>;
  kpis: Record<string, jest.Mock>;
  controller: VendedorFiscalController;
} {
  const tributacion = { sujetoFiscalDe: jest.fn() };
  const kpis = { resumenDelVendedor: jest.fn() };
  const controller = new VendedorFiscalController(
    tributacion as unknown as TributacionService,
    kpis as unknown as KpisService,
  );
  return { tributacion, kpis, controller };
}

describe('VendedorFiscalController (api/v1/vendedores/me/fiscal, R-U-10/11)', () => {
  it('debe componer la situacion fiscal con sujeto y resumen de comisiones del mes', async () => {
    const { tributacion, kpis, controller } = fabricar();
    tributacion.sujetoFiscalDe.mockResolvedValue({ id: 's-1', regimen: 'Cuota Fija' });
    kpis.resumenDelVendedor.mockResolvedValue({ comisiones_cents: 13800 });
    const resultado = await controller.situacion(vendedor as never, '2026-09');
    expect(tributacion.sujetoFiscalDe).toHaveBeenCalledWith('u-v');
    expect(kpis.resumenDelVendedor).toHaveBeenCalledWith('u-v', '2026-09');
    expect(resultado.sujeto_fiscal).toEqual({ id: 's-1', regimen: 'Cuota Fija' });
    expect(resultado.nota).toContain('Ley 822');
  });

  it('debe devolver sujeto_fiscal null cuando el vendedor no tiene ficha fiscal', async () => {
    const { tributacion, kpis, controller } = fabricar();
    tributacion.sujetoFiscalDe.mockResolvedValue(null);
    kpis.resumenDelVendedor.mockResolvedValue({ comisiones_cents: 0 });
    const resultado = await controller.situacion(vendedor as never, undefined);
    expect(resultado.sujeto_fiscal).toBeNull();
    expect(kpis.resumenDelVendedor).toHaveBeenCalledWith('u-v', undefined);
  });

  it('debe exigir rol vendedor a nivel de controlador', () => {
    expect(Reflect.getMetadata('roles_requeridos', VendedorFiscalController)).toEqual([ROLES.VENDEDOR]);
  });
});