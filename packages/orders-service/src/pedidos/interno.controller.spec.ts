import 'reflect-metadata';
import { InternoController } from './interno.controller';
import type { CreateOrderCommandHandler } from './handlers/create-order-command.handler';
import type { OrderViewRepository } from './queries/order-view.repository';
import type { OrderEventStore } from './repositories/order-event-store';
import type { PedidosService } from './pedidos.service';
import { ClaveInternaGuard, NotFoundError } from '@core/shared';

const orden = { id: 'o-1', cliente_id: 'u-c', estado: 'creada', total_cents: 1000 };
const evento = {
  event_id: 'ev-1',
  aggregate_id: 'o-1',
  tipo: 'order.created',
  payload: { total_cents: 1000 },
  version: 1,
  creado_en: '2026-01-01T00:00:00.000Z',
};

function fabricar(): {
  handler: Record<string, jest.Mock>;
  views: Record<string, jest.Mock>;
  eventStore: Record<string, jest.Mock>;
  pedidos: Record<string, jest.Mock>;
  controller: InternoController;
} {
  const handler = { transicionar: jest.fn(), reproyectar: jest.fn(), ejecutar: jest.fn() };
  const views = { listarTodo: jest.fn(), encontrar: jest.fn() };
  const eventStore = { historiaDe: jest.fn() };
  const pedidos = { detallePara: jest.fn() };
  const controller = new InternoController(
    handler as unknown as CreateOrderCommandHandler,
    views as unknown as OrderViewRepository,
    eventStore as unknown as OrderEventStore,
    pedidos as unknown as PedidosService,
  );
  return { handler, views, eventStore, pedidos, controller };
}

describe('InternoController (internal, R-U-10/11)', () => {
  it('debe transicionar la orden cuando llega el estado', async () => {
    const { handler, controller } = fabricar();
    handler.transicionar.mockResolvedValue(orden);
    await expect(controller.transicion('o-1', 'pagada', 'pago ok')).resolves.toEqual(orden);
    expect(handler.transicionar).toHaveBeenCalledWith('o-1', 'pagada', 'pago ok', 'logistica');
  });

  it('debe lanzar NotFoundError Estado requerido cuando falta el estado', async () => {
    const { controller } = fabricar();
    await expect(controller.transicion('o-1', '', undefined)).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe devolver la orden cuando existe y lanzar NotFoundError cuando no', async () => {
    const { views, controller } = fabricar();
    views.encontrar.mockResolvedValue(orden);
    await expect(controller.orden('o-1')).resolves.toEqual(orden);
    views.encontrar.mockResolvedValue(null);
    await expect(controller.orden('o-1')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe listar todas o filtrar por estado', async () => {
    const { views, controller } = fabricar();
    views.listarTodo.mockResolvedValue([orden]);
    await controller.listar(undefined);
    expect(views.listarTodo).toHaveBeenCalledWith({});
    await controller.listar('cancelada');
    expect(views.listarTodo).toHaveBeenCalledWith({ estado: 'cancelada' });
  });

  it('debe reproyectar la orden desde la historia', async () => {
    const { handler, controller } = fabricar();
    handler.reproyectar.mockResolvedValue(orden);
    await expect(controller.reproyectar('o-1')).resolves.toEqual(orden);
    expect(handler.reproyectar).toHaveBeenCalledWith('o-1');
  });

  it('debe devolver la historia mapeada cuando hay eventos', async () => {
    const { eventStore, controller } = fabricar();
    eventStore.historiaDe.mockResolvedValue([evento]);
    const historia = await controller.historia('o-1');
    expect(historia).toEqual([
      { event_id: 'ev-1', aggregate_id: 'o-1', tipo: 'order.created', payload: { total_cents: 1000 }, version: 1, creado_en: '2026-01-01T00:00:00.000Z' },
    ]);
  });

  it('debe lanzar NotFoundError cuando la orden no tiene historia', async () => {
    const { eventStore, controller } = fabricar();
    eventStore.historiaDe.mockResolvedValue([]);
    await expect(controller.historia('o-x')).rejects.toBeInstanceOf(NotFoundError);
  });

  it('debe proteger todas las rutas con la clave interna', () => {
    const guards = Reflect.getMetadata('__guards__', InternoController) as unknown[];
    expect(guards.some((g) => g === ClaveInternaGuard)).toBe(true);
  });
});