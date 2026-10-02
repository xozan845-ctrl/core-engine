import 'reflect-metadata';
import { MetricasController } from './metricas.controller';
import type { InteligenciaService } from '../services/inteligencia.service';

function fabricar(): { inteligencia: Record<string, jest.Mock>; controller: MetricasController } {
  const inteligencia = {
    obtenerMapaCalor: jest.fn(),
    obtenerRendimientoVendedores: jest.fn(),
    obtenerCoberturaZona: jest.fn(),
    obtenerDemandaProductos: jest.fn(),
    obtenerTendencias: jest.fn(),
    obtenerResumen: jest.fn(),
    registrarVenta: jest.fn(),
  };
  const controller = new MetricasController(inteligencia as unknown as InteligenciaService);
  return { inteligencia, controller };
}

describe('MetricasController (api/v1/inteligencia, R-U-10/11)', () => {
  it('debe devolver el mapa de calor con los filtros recibidos', async () => {
    const { inteligencia, controller } = fabricar();
    inteligencia.obtenerMapaCalor.mockResolvedValue({ puntos: [] });
    const filtros = { fecha_inicio: '2026-09-01', fecha_fin: '2026-09-30' };
    await expect(controller.getMapaCalor(filtros as never)).resolves.toEqual({ puntos: [] });
    expect(inteligencia.obtenerMapaCalor).toHaveBeenCalledWith(filtros);
  });

  it('debe inyectar el vendedor del header en el mapa de calor propio', async () => {
    const { inteligencia, controller } = fabricar();
    inteligencia.obtenerMapaCalor.mockResolvedValue({ puntos: [] });
    await controller.getMiMapaCalor('personal-7', { zona: 'norte' } as never);
    expect(inteligencia.obtenerMapaCalor).toHaveBeenCalledWith({ zona: 'norte', vendedor_id: 'personal-7' });
  });

  it('debe delegar rendimiento y su variante propia con el vendedor del header', async () => {
    const { inteligencia, controller } = fabricar();
    inteligencia.obtenerRendimientoVendedores.mockResolvedValue({ items: [] });
    await controller.getRendimiento({ limite: 10 } as never);
    expect(inteligencia.obtenerRendimientoVendedores).toHaveBeenCalledWith({ limite: 10 });
    await controller.getMiRendimiento('personal-7', { limite: 10 } as never);
    expect(inteligencia.obtenerRendimientoVendedores).toHaveBeenCalledWith({ limite: 10, vendedor_id: 'personal-7' });
  });

  it('debe delegar cobertura, demanda, tendencias y resumen', async () => {
    const { inteligencia, controller } = fabricar();
    inteligencia.obtenerCoberturaZona.mockResolvedValue({ zonas: [] });
    inteligencia.obtenerDemandaProductos.mockResolvedValue({ items: [] });
    inteligencia.obtenerTendencias.mockResolvedValue({ series: [] });
    inteligencia.obtenerResumen.mockResolvedValue({ total: 0 });
    await controller.getCobertura({ zona: 'norte' });
    await controller.getDemanda({ limite: 5 } as never);
    await controller.getTendencias({ fecha_inicio: '2026-09-01' } as never);
    await controller.getResumen({} as never);
    expect(inteligencia.obtenerCoberturaZona).toHaveBeenCalledWith({ zona: 'norte' });
    expect(inteligencia.obtenerDemandaProductos).toHaveBeenCalledWith({ limite: 5 });
    expect(inteligencia.obtenerTendencias).toHaveBeenCalledWith({ fecha_inicio: '2026-09-01' });
    expect(inteligencia.obtenerResumen).toHaveBeenCalledWith({});
  });

  it('debe registrar la venta y devolver el acuse OK', async () => {
    const { inteligencia, controller } = fabricar();
    inteligencia.registrarVenta.mockResolvedValue(undefined);
    const data = { order_id: 'o-1', monto_cents: 1000, lat: 12, lng: -86 };
    await expect(controller.registrarVenta(data)).resolves.toEqual({ codigo: 'OK', mensaje: 'Venta registrada en inteligencia' });
    expect(inteligencia.registrarVenta).toHaveBeenCalledWith(data);
  });
});