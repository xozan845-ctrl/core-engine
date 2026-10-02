import 'reflect-metadata';
import { TributacionController } from './tributacion.controller';
import type { TributacionService } from './tributacion.service';
import { NotFoundError, ROLES } from '@core/shared';

const jurisdiccion = { codigo_pais: 'NI', nombre: 'Nicaragua', moneda: 'NIO' };
const declaracion = { id: 'd-1', tipo: 'IVA', periodo_inicio: '2026-09-01', estado: 'pendiente' };

function fabricar(): { tributacion: Record<string, jest.Mock>; controller: TributacionController } {
  const tributacion = {
    listarJurisdicciones: jest.fn(),
    crearJurisdiccion: jest.fn(),
    regimenesDe: jest.fn(),
    crearRegimen: jest.fn(),
    sujetos: jest.fn(),
    registrarSujeto: jest.fn(),
    darDeBaja: jest.fn(),
    declaraciones: jest.fn(),
    generarDeclaraciones: jest.fn(),
    presentar: jest.fn(),
    marcarPagada: jest.fn(),
  };
  const controller = new TributacionController(tributacion as unknown as TributacionService);
  return { tributacion, controller };
}

describe('TributacionController (api/v1/finanzas, Ley 822, R-U-10/11)', () => {
  it('debe listar y crear jurisdicciones convirtiendo tasas de por-mil a fraccion', async () => {
    const { tributacion, controller } = fabricar();
    tributacion.listarJurisdicciones.mockResolvedValue([jurisdiccion]);
    await expect(controller.jurisdicciones()).resolves.toEqual([jurisdiccion]);
    tributacion.crearJurisdiccion.mockResolvedValue(jurisdiccion);
    await controller.crearJurisdiccion({
      codigo_pais: 'NI',
      nombre: 'Nicaragua',
      moneda: 'NIO',
      simbolo_moneda: 'C$',
      tasa_iva_por_mil: 150,
      tasa_ir_por_mil: 300,
    } as never);
    expect(tributacion.crearJurisdiccion).toHaveBeenCalledWith(
      expect.objectContaining({ tasa_iva: 0.15, tasa_ir: 0.3 }),
    );
  });

  it('debe listar regimenes y crear uno con NotFoundError cuando no se resuelve', async () => {
    const { tributacion, controller } = fabricar();
    tributacion.regimenesDe.mockResolvedValue([]);
    await controller.regimenes('NI');
    expect(tributacion.regimenesDe).toHaveBeenCalledWith('NI');
    tributacion.crearRegimen.mockResolvedValue({ codigo: 'CF' });
    await expect(controller.crearRegimen({ jurisdiccion: 'NI', codigo: 'CF', nombre: 'Cuota Fija' } as never)).resolves.toEqual({ codigo: 'CF' });
    tributacion.crearRegimen.mockResolvedValue(null);
    await expect(controller.crearRegimen({ jurisdiccion: 'NI', codigo: 'CF', nombre: 'Cuota Fija' } as never)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe registrar sujeto y darlo de baja con sus NotFoundError', async () => {
    const { tributacion, controller } = fabricar();
    tributacion.sujetos.mockResolvedValue([]);
    await controller.sujetos();
    tributacion.registrarSujeto.mockResolvedValue({ id: 's-1' });
    await expect(controller.registrarSujeto({ id: 's-1', razon_social: 'ACME' } as never)).resolves.toEqual({ id: 's-1' });
    tributacion.registrarSujeto.mockResolvedValue(null);
    await expect(controller.registrarSujeto({ id: 's-1', razon_social: 'ACME' } as never)).rejects.toBeInstanceOf(NotFoundError);
    tributacion.darDeBaja.mockResolvedValue(null);
    await expect(controller.darDeBaja('s-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe filtrar declaraciones y generar con periodo opcional', async () => {
    const { tributacion, controller } = fabricar();
    tributacion.declaraciones.mockResolvedValue([declaracion]);
    await controller.declaraciones('IVA', '2026-09-01', 'pendiente');
    expect(tributacion.declaraciones).toHaveBeenCalledWith({ tipo: 'IVA', periodo_inicio: '2026-09-01', estado: 'pendiente' });
    tributacion.generarDeclaraciones.mockResolvedValue([]);
    await controller.generar({ inicio: '2026-09-01', fin: '2026-09-30' } as never);
    expect(tributacion.generarDeclaraciones).toHaveBeenCalledWith({ inicio: '2026-09-01', fin: '2026-09-30' });
    await controller.generar({} as never);
    expect(tributacion.generarDeclaraciones).toHaveBeenCalledWith(undefined);
  });

  it('debe presentar y pagar declaraciones con NotFoundError', async () => {
    const { tributacion, controller } = fabricar();
    tributacion.presentar.mockResolvedValue(declaracion);
    await expect(controller.presentar('d-1')).resolves.toEqual(declaracion);
    tributacion.marcarPagada.mockResolvedValue(null);
    await expect(controller.pagar('d-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe exigir rol admin a nivel de controlador', () => {
    expect(Reflect.getMetadata('roles_requeridos', TributacionController)).toEqual([ROLES.ADMIN]);
  });
});