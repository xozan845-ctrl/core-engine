import type { Mock } from 'vitest';
import 'reflect-metadata';
import { PedidosController } from './pedidos.controller';
import type { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import type { OrderViewRepository } from './queries/order-view.repository';
import type { PedidosService } from './pedidos.service';
import { ROLES } from '@core/shared';

const orden = { id: 'o-1', cliente_id: 'u-c', estado: 'creada', total_cents: 1000 };
const comprador = { user_id: 'u-c', email: 'c@core.local', rol: ROLES.COMPRADOR };
const vendedor = { user_id: 'u-v', email: 'v@core.local', rol: ROLES.VENDEDOR };
const admin = { user_id: 'u-a', email: 'a@core.local', rol: ROLES.ADMIN };
const superAdmin = { user_id: 'u-sa', email: 'sa@core.local', rol: ROLES.SUPER_ADMIN };

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
    obtenerComisiones: vi.fn(),
    listarDeTienda: vi.fn(),
    listarDeCliente: vi.fn(),
    obtenerTimeline: vi.fn(),
  };
  const pedidos = { detallePara: vi.fn() };
  const controller = new PedidosController(
    handler as unknown as CreateOrderCommandHandler,
    views as unknown as OrderViewRepository,
    pedidos as unknown as PedidosService,
  );
  return { handler, views, pedidos, controller };
}

describe('PedidosController (api/v1/orders, R-U-10/11)', () => {
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

  it('debe pedir las comisiones del vendedor autenticado cuando no es admin', async () => {
    const { views, controller } = fabricar();
    views.obtenerComisiones.mockResolvedValue([]);
    await controller.comisiones('2026-09', vendedor as never, undefined);
    expect(views.obtenerComisiones).toHaveBeenCalledWith('u-v', '2026-09');
  });

  it('debe pedir las comisiones de otro vendedor cuando el contexto es admin', async () => {
    const { views, controller } = fabricar();
    views.obtenerComisiones.mockResolvedValue([]);
    await controller.comisiones('2026-09', admin as never, 'u-v2');
    expect(views.obtenerComisiones).toHaveBeenCalledWith('u-v2', '2026-09');
  });

  it('debe pedir las comisiones de otro vendedor cuando el contexto es super_admin', async () => {
    const { views, controller } = fabricar();
    views.obtenerComisiones.mockResolvedValue([]);
    await controller.comisiones('2026-09', superAdmin as never, 'u-v2');
    expect(views.obtenerComisiones).toHaveBeenCalledWith('u-v2', '2026-09');
  });

  it('debe listar las ordenes de la tienda pasando el vendedor solo cuando es vendedor', async () => {
    const { views, controller } = fabricar();
    views.listarDeTienda.mockResolvedValue([orden]);
    await controller.porTienda('tienda-1', 'creada', vendedor as never);
    expect(views.listarDeTienda).toHaveBeenCalledWith('tienda-1', 'creada', 'u-v');
    await controller.porTienda('tienda-1', undefined, admin as never);
    expect(views.listarDeTienda).toHaveBeenCalledWith('tienda-1', undefined, undefined);
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

  it('debe devolver el timeline tras resolver la orden', async () => {
    const { pedidos, views, controller } = fabricar();
    pedidos.detallePara.mockResolvedValue(orden);
    views.obtenerTimeline.mockResolvedValue([{ tipo: 'order.created', version: 1 }]);
    await controller.timeline('o-1', comprador as never);
    expect(views.obtenerTimeline).toHaveBeenCalledWith('o-1');
  });

  it('debe exigir rol comprador en crear y vendedor, admin o super_admin en comisiones', () => {
    expect(Reflect.getMetadata('roles_requeridos', PedidosController.prototype.crear)).toEqual([ROLES.COMPRADOR]);
    expect(Reflect.getMetadata('roles_requeridos', PedidosController.prototype.comisiones)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.VENDEDOR,
      ROLES.ADMIN,
    ]);
  });
});