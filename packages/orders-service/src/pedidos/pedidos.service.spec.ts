import type { Mock } from 'vitest';
import 'reflect-metadata';
import { ForbiddenError, NotFoundError, ROLES } from '@core/shared';
import { PedidosService } from './pedidos.service';
import type { OrderViewRepository, OrderView } from './queries/order-view.repository';

const orden = {
  id: 'o-1',
  vendedor_id: 'u-v',
  comprador_id: 'u-c',
  tienda_id: 't-1',
  estado: 'pagada',
  total_cents: 1000,
  comision_cents: 120,
  moneda: 'NIO',
  items: [{ vendedor_id: 'u-v', empresa_id: 'e-1' }],
  direccion_envio: null,
  creado_en: '2026-01-01T00:00:00.000Z',
  actualizado_en: '2026-01-01T00:00:00.000Z',
  pagada_en: null,
  enviada_en: null,
  entregada_en: null,
  cancelada_en: null,
  devuelta_en: null,
} as unknown as OrderView;

function fabricar(): { views: Record<string, Mock>; servicio: PedidosService } {
  const views = { encontrar: vi.fn() };
  const servicio = new PedidosService(views as unknown as OrderViewRepository);
  return { views, servicio };
}

describe('PedidosService.detallePara (Tabla 15, ADR-25)', () => {
  it('debe permitir al admin global ver cualquier orden', async () => {
    const { views, servicio } = fabricar();
    views.encontrar.mockResolvedValue(orden);
    await expect(
      servicio.detallePara('o-1', { user_id: 'u-a', email: 'a@test', rol: ROLES.ADMIN }),
    ).resolves.toEqual(orden);
  });

  it('debe permitir al super_admin ver cualquier orden', async () => {
    const { views, servicio } = fabricar();
    views.encontrar.mockResolvedValue(orden);
    await expect(
      servicio.detallePara('o-1', { user_id: 'u-sa', email: 'sa@test', rol: ROLES.SUPER_ADMIN }),
    ).resolves.toEqual(orden);
  });

  it('debe permitir al comprador ver solo su propia orden', async () => {
    const { views, servicio } = fabricar();
    views.encontrar.mockResolvedValue(orden);
    await expect(
      servicio.detallePara('o-1', { user_id: 'u-c', email: 'c@test', rol: ROLES.COMPRADOR }),
    ).resolves.toEqual(orden);
    await expect(
      servicio.detallePara('o-1', { user_id: 'u-otro', email: 'otro@test', rol: ROLES.COMPRADOR }),
    ).rejects.toBeInstanceOf(ForbiddenError);
  });

  it('debe permitir al vendedor ver una orden que contiene sus items', async () => {
    const { views, servicio } = fabricar();
    views.encontrar.mockResolvedValue(orden);
    await expect(
      servicio.detallePara('o-1', { user_id: 'u-v', email: 'v@test', rol: ROLES.VENDEDOR }),
    ).resolves.toEqual(orden);
    await expect(
      servicio.detallePara('o-1', { user_id: 'u-otro', email: 'otro@test', rol: ROLES.VENDEDOR }),
    ).rejects.toBeInstanceOf(ForbiddenError);
  });

  it('debe lanzar NotFoundError cuando la orden no existe', async () => {
    const { views, servicio } = fabricar();
    views.encontrar.mockResolvedValue(null);
    await expect(
      servicio.detallePara('o-x', { user_id: 'u-a', email: 'a@test', rol: ROLES.ADMIN }),
    ).rejects.toBeInstanceOf(NotFoundError);
  });
});
