import type { Mock } from 'vitest';
import 'reflect-metadata';
import { Module } from '@nestjs/common';
import type { INestApplication } from '@nestjs/common';
import { NestFactory } from '@nestjs/core';
import { FinanzasController } from './finanzas.controller';
import { ProyeccionesService } from './proyecciones.service';
import { KpisService } from './kpis.service';
import { AlertasService } from './alertas.service';
import { NotFoundError, ROLES } from '@core/shared';

const proyeccion = { id: 'pr-1', nombre: 'Base', supuestos: {}, resultado: {} };
const indicadores = {
  punto_equilibrio_caja_cents: 100000,
  pedidos_equilibrio_caja: 200,
  escenarios_equilibrio: {},
  margen_seguridad_por_mes: [],
};

function fabricar(): {
  proyecciones: Record<string, Mock>;
  kpis: Record<string, Mock>;
  alertas: Record<string, Mock>;
  controller: FinanzasController;
} {
  const proyecciones = {
    crear: vi.fn(),
    listar: vi.fn(),
    obtener: vi.fn(),
    calcular: vi.fn(),
    indicadores: vi.fn(),
    sensibilidadComisionGmv: vi.fn(),
    sensibilidadVan: vi.fn(),
    paybackPorEscenario: vi.fn(),
    coberturaPorVendedor: vi.fn(),
    planBienal: vi.fn(),
  };
  const kpis = { kpis: vi.fn() };
  const alertas = { tablero: vi.fn() };
  const controller = new FinanzasController(
    proyecciones as unknown as ProyeccionesService,
    kpis as unknown as KpisService,
    alertas as unknown as AlertasService,
  );
  return { proyecciones, kpis, alertas, controller };
}

const proyeccionesHttp = {
  calcular: vi.fn().mockReturnValue({ filas: [] }),
  sensibilidadComisionGmv: vi.fn().mockReturnValue([]),
  sensibilidadVan: vi.fn().mockReturnValue([]),
  paybackPorEscenario: vi.fn().mockReturnValue([]),
  coberturaPorVendedor: vi.fn().mockReturnValue([]),
  planBienal: vi.fn().mockReturnValue({ rois: [] }),
  obtener: vi.fn().mockResolvedValue(null),
};

@Module({
  controllers: [FinanzasController],
  providers: [
    { provide: ProyeccionesService, useValue: proyeccionesHttp },
    { provide: KpisService, useValue: { kpis: vi.fn() } },
    { provide: AlertasService, useValue: { tablero: vi.fn() } },
  ],
})
class FinanzasHttpTestModule {}

describe('FinanzasController — orden de rutas (R-DO-9)', () => {
  let app: INestApplication;
  let puerto: number;

  beforeAll(async () => {
    app = await NestFactory.create(FinanzasHttpTestModule, { logger: false });
    await app.listen(0);
    puerto = (app.getHttpServer().address() as { port: number }).port;
  });

  afterAll(async () => {
    await app.close();
  });

  it('debe resolver proyecciones/sensibilidad cuando compite con proyecciones/:id', async () => {
    const respuesta = await fetch(
      `http://127.0.0.1:${puerto}/api/v1/finanzas/proyecciones/sensibilidad`,
    );
    expect(respuesta.status).toBe(200);
    const cuerpo = (await respuesta.json()) as {
      supuestos?: unknown;
      matriz_comision_gmv?: unknown;
    };
    expect(cuerpo.supuestos).toBeDefined();
    expect(cuerpo.matriz_comision_gmv).toBeDefined();
  });

  it('debe resolver proyecciones/plan-bienal cuando compite con proyecciones/:id', async () => {
    const respuesta = await fetch(
      `http://127.0.0.1:${puerto}/api/v1/finanzas/proyecciones/plan-bienal`,
    );
    expect(respuesta.status).toBe(200);
    await expect(respuesta.json()).resolves.toEqual({ rois: [] });
  });
});

describe('FinanzasController (api/v1/finanzas, cap. 8, R-U-10/11)', () => {
  it('debe crear una proyeccion con nombre y supuestos', async () => {
    const { proyecciones, controller } = fabricar();
    proyecciones.crear.mockResolvedValue(proyeccion);
    const supuestos = { horizonte_meses: 12 } as never;
    await expect(controller.crear({ nombre: 'Base', supuestos } as never)).resolves.toEqual(proyeccion);
    expect(proyecciones.crear).toHaveBeenCalledWith('Base', supuestos);
  });

  it('debe listar proyecciones y devolver el detalle o NotFoundError', async () => {
    const { proyecciones, controller } = fabricar();
    proyecciones.listar.mockResolvedValue([proyeccion]);
    await expect(controller.listar()).resolves.toEqual([proyeccion]);
    proyecciones.obtener.mockResolvedValue(proyeccion);
    await expect(controller.detalle('pr-1')).resolves.toEqual(proyeccion);
    proyecciones.obtener.mockResolvedValue(null);
    await expect(controller.detalle('pr-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe calcular el punto de equilibrio con los supuestos por defecto', async () => {
    const { proyecciones, controller } = fabricar();
    proyecciones.calcular.mockReturnValue({ filas: [] });
    proyecciones.indicadores.mockReturnValue(indicadores);
    const resultado = await controller.puntoEquilibrio(undefined, undefined, undefined, undefined);
    expect(proyecciones.calcular).toHaveBeenCalledWith(
      expect.objectContaining({ horizonte_meses: 12, ticket_promedio_cents: 45000, comision_tasa: 0.12 }),
    );
    expect(resultado.punto_equilibrio_caja_cents).toBe(100000);
  });

  it('debe usar los parametros de la query en el punto de equilibrio', async () => {
    const { proyecciones, controller } = fabricar();
    proyecciones.calcular.mockReturnValue({ filas: [] });
    proyecciones.indicadores.mockReturnValue(indicadores);
    await controller.puntoEquilibrio('500000', '60000', '0.2', '6');
    expect(proyecciones.calcular).toHaveBeenCalledWith(
      expect.objectContaining({ horizonte_meses: 6, ticket_promedio_cents: 60000, comision_tasa: 0.2, costos_fijos_cents: 500000 }),
    );
  });

  it('debe devolver los KPIs del mes', async () => {
    const { kpis, controller } = fabricar();
    kpis.kpis.mockResolvedValue({ margen_bruto_cents: 1 });
    await expect(controller.kpisDelMes('2026-09')).resolves.toEqual({ margen_bruto_cents: 1 });
    expect(kpis.kpis).toHaveBeenCalledWith('2026-09');
  });

  it('debe componer el estudio de sensibilidad', async () => {
    const { proyecciones, controller } = fabricar();
    proyecciones.calcular.mockReturnValue({ filas: [] });
    proyecciones.sensibilidadComisionGmv.mockReturnValue([]);
    proyecciones.sensibilidadVan.mockReturnValue([]);
    proyecciones.paybackPorEscenario.mockReturnValue([]);
    proyecciones.coberturaPorVendedor.mockReturnValue([]);
    const estudio = await controller.sensibilidad();
    expect(estudio.supuestos.horizonte_meses).toBe(12);
    expect(proyecciones.sensibilidadVan).toHaveBeenCalledWith([], expect.any(Object));
  });

  it('debe delegar el plan bienal y el tablero con conversion numerica', async () => {
    const { proyecciones, alertas, controller } = fabricar();
    proyecciones.planBienal.mockReturnValue({ rois: [] });
    alertas.tablero.mockResolvedValue({ kpis: {}, alertas: [] });
    await expect(controller.planBienal()).resolves.toEqual({ rois: [] });
    await controller.tablero('2026-09', '500');
    expect(alertas.tablero).toHaveBeenCalledWith({ mes: '2026-09', costo_entrega_por_pedido_cents: 500 });
    await controller.tablero(undefined, undefined);
    expect(alertas.tablero).toHaveBeenCalledWith({ mes: undefined, costo_entrega_por_pedido_cents: undefined });
  });

  it('debe exigir rol admin a nivel de controlador', () => {
    expect(Reflect.getMetadata('roles_requeridos', FinanzasController)).toEqual([ROLES.ADMIN]);
  });
});