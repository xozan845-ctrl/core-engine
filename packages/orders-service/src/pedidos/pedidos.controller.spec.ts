import type { Mock } from 'vitest';
import 'reflect-metadata';
import { PedidosController } from './pedidos.controller';
import type { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import type { OrderViewRepository } from './queries/order-view.repository';
import type { PedidosService } from './pedidos.service';
import { ROLES } from '@core/shared';

const orden = { id: 'o-1', comprador_id: 'u-c', estado: 'creada', total_cents: 1000 };
const comprador = { user_id: 'u-c', email: 'c@core.local', rol: ROLES.COMPRADOR };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };
const admin = { user_id: 'u-a', email: 'a@core.local', rol: ROLES.ADMIN };
const empresario = { user_id: 'u-e', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 'e-1' };

function fabricar(): {
  handler: Record<string, Mock>;
  views: Record<string, Mock>;
  pedidos: Record<string, Mock>;
  controller: PedidosController;
} {
  const handler = { ejecutar: vi.fn(), transicionar: vi.fn(), reproyectar: vi.fn() };
  const views = {
    listarTodo: vi.fn(),
    listarDeVendedor: vi.fn(),
    listarDeTienda: vi.fn(),
    listarDeEmpresa: vi.fn(),
    listarDeCliente: vi.fn(),
    obtenerTimeline: vi.fn(),
  };
  const pedidos = {
    detallePara: vi.fn(),
    paraEmpresa: vi.fn((orden: unknown) => orden),
    paraVendedor: vi.fn((orden: unknown) => orden),
    timelineVisible: vi.fn((eventos: unknown) => eventos),
  };
  const controller = new PedidosController(
    handler as unknown as CreateOrderCommandHandler,
    views as unknown as OrderViewRepository,
    pedidos as unknown as PedidosService,
  );
  return { handler, views, pedidos, controller };
}

describe('PedidosController (api/v2/orders, R-U-10/11)', () => {
  it('debe crear la orden ejecutando el comando con el id del comprador', async () => {
    const { handler, controller } = fabricar();
    handler.ejecutar.mockResolvedValue(orden);
    const comando = { items: [{ oferta_id: 'of-1', cantidad: 2 }] };
    await expect(controller.crear(comando as never, comprador as never)).resolves.toEqual(orden);
    expect(handler.ejecutar).toHaveBeenCalledWith(comando, 'u-c');
  });

  it('debe listar todas las ordenes con filtro de estado y limite', async () => {
    const { views, controller } = fabricar();
    views.listarTodo.mockResolvedValue([orden]);
    await controller.todas('pagada', 10);
    expect(views.listarTodo).toHaveBeenCalledWith({ estado: 'pagada', limite: 10 });
  });

  it('debe listar las ventas del vendedor autenticado', async () => {
    const { views, controller } = fabricar();
    views.listarDeVendedor.mockResolvedValue([orden]);
    await controller.misVentas('enviada', vendedor as never);
    expect(views.listarDeVendedor).toHaveBeenCalledWith('u-v', 'enviada');
  });

  it('debe listar las ordenes de la tienda pasando vendedor o empresa segun el contexto', async () => {
    const { views, controller } = fabricar();
    views.listarDeTienda.mockResolvedValue([orden]);
    await controller.porTienda('tienda-1', 'creada', vendedor as never);
    expect(views.listarDeTienda).toHaveBeenCalledWith('tienda-1', 'creada', 'u-v', undefined);
    await controller.porTienda('tienda-1', undefined, admin as never);
    expect(views.listarDeTienda).toHaveBeenCalledWith('tienda-1', undefined, undefined, undefined);
    await controller.porTienda('tienda-1', undefined, empresario as never);
    expect(views.listarDeTienda).toHaveBeenCalledWith('tienda-1', undefined, undefined, 'e-1');
  });

  it('debe listar las ordenes de la empresa activa del empresario y recortarlas', async () => {
    const { views, pedidos, controller } = fabricar();
    views.listarDeEmpresa.mockResolvedValue([orden]);
    await controller.porEmpresa('e-otra', 'tienda-1', 'pagada', 20, empresario as never);
    expect(views.listarDeEmpresa).toHaveBeenCalledWith('e-1', {
      estado: 'pagada',
      tiendaId: 'tienda-1',
      limite: 20,
    });
    expect(pedidos.paraEmpresa).toHaveBeenCalledWith(orden, 'e-1');
  });

  it('debe exigir empresa al admin y respetar el filtro solicitado', async () => {
    const { views, pedidos, controller } = fabricar();
    views.listarDeEmpresa.mockResolvedValue([orden]);
    await controller.porEmpresa('e-2', undefined, undefined, undefined, admin as never);
    expect(views.listarDeEmpresa).toHaveBeenCalledWith('e-2', {
      estado: undefined,
      tiendaId: undefined,
      limite: undefined,
    });
    expect(pedidos.paraEmpresa).not.toHaveBeenCalled();
    await expect(
      controller.porEmpresa(undefined, undefined, undefined, undefined, admin as never),
    ).rejects.toHaveProperty('codigo', 'EMPRESA_REQUERIDA');
  });

  it('debe rechazar al empresario sin empresa activa al listar ordenes de una empresa', async () => {
    const { views, controller } = fabricar();
    await expect(
      controller.porEmpresa(undefined, undefined, undefined, undefined, {
        user_id: 'u-e',
        email: 'e@core.local',
        rol: ROLES.EMPRESARIO,
      } as never),
    ).rejects.toHaveProperty('codigo', 'TENANT_REQUERIDO');
    expect(views.listarDeEmpresa).not.toHaveBeenCalled();
  });

  it('debe rechazar al empresario sin empresa activa al listar ordenes de una tienda', async () => {
    const { views, controller } = fabricar();
    await expect(
      controller.porTienda('tienda-1', undefined, {
        user_id: 'u-e',
        email: 'e@core.local',
        rol: ROLES.EMPRESARIO,
      } as never),
    ).rejects.toHaveProperty('codigo', 'TENANT_REQUERIDO');
    expect(views.listarDeTienda).not.toHaveBeenCalled();
  });

  it('debe listar el historial del comprador autenticado', async () => {
    const { views, controller } = fabricar();
    views.listarDeCliente.mockResolvedValue([orden]);
    await controller.misOrdenes('creada', comprador as never);
    expect(views.listarDeCliente).toHaveBeenCalledWith('u-c', 'creada');
  });

  it('debe devolver el detalle de la orden para el contexto', async () => {
    const { pedidos, controller } = fabricar();
    pedidos.detallePara.mockResolvedValue(orden);
    await expect(controller.detalle('o-1', comprador as never)).resolves.toEqual(orden);
    expect(pedidos.detallePara).toHaveBeenCalledWith('o-1', comprador);
  });

  it('debe devolver el timeline visible tras resolver la orden', async () => {
    const { pedidos, views, controller } = fabricar();
    const eventos = [{ tipo: 'order.created', version: 1 }];
    pedidos.detallePara.mockResolvedValue(orden);
    views.obtenerTimeline.mockResolvedValue(eventos);
    await expect(controller.timeline('o-1', comprador as never)).resolves.toBe(eventos);
    expect(views.obtenerTimeline).toHaveBeenCalledWith('o-1');
    expect(pedidos.timelineVisible).toHaveBeenCalledWith(eventos, orden.items);
  });

  it('debe exigir rol comprador en crear y los roles esperados en las consultas', () => {
    expect(Reflect.getMetadata('roles_requeridos', PedidosController.prototype.crear)).toEqual([ROLES.COMPRADOR]);
    expect(Reflect.getMetadata('roles_requeridos', PedidosController.prototype.detalle)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.EMPRESARIO,
      ROLES.COMPRADOR,
      ROLES.VENDEDOR,
      ROLES.ADMIN,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', PedidosController.prototype.porEmpresa)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.EMPRESARIO,
      ROLES.ADMIN,
    ]);
  });
});