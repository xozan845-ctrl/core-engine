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
  items: [
    {
      oferta_id: 'of-1',
      sku: 'SKU-1',
      producto_nombre: 'Producto 1',
      cantidad: 1,
      precio_unitario_cents: 1000,
      vendedor_id: 'u-v',
      tienda_id: 't-1',
      empresa_id: 'e-1',
      participacion_porcentaje: 20,
    },
  ],
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

  it('debe permitir al empresario ver una orden de su empresa', async () => {
    const { views, servicio } = fabricar();
    views.encontrar.mockResolvedValue(orden);
    await expect(
      servicio.detallePara('o-1', {
        user_id: 'u-e',
        email: 'e@test',
        rol: ROLES.EMPRESARIO,
        tenant_id: 'e-1',
      }),
    ).resolves.toEqual(orden);
  });

  it('debe recortar las líneas de otras empresas al empresario', async () => {
    const { views, servicio } = fabricar();
    const compartida = {
      ...orden,
      total_cents: 3000,
      comision_cents: 360,
      items: [
        ...orden.items,
        {
          ...orden.items[0],
          oferta_id: 'of-2',
          sku: 'SKU-2',
          precio_unitario_cents: 2000,
          vendedor_id: 'u-v2',
          tienda_id: 't-2',
          empresa_id: 'e-2',
        },
      ],
    } as OrderView;
    views.encontrar.mockResolvedValue(compartida);

    const resultado = await servicio.detallePara('o-1', {
      user_id: 'u-e',
      email: 'e@test',
      rol: ROLES.EMPRESARIO,
      tenant_id: 'e-1',
    });

    expect(resultado.items.map((i) => i.oferta_id)).toEqual(['of-1']);
    expect(resultado.total_cents).toBe(1000);
    expect(resultado.comision_cents).toBe(120);
    expect(resultado.vendedor_id).toBe('u-v');
  });

  it('debe recortar las líneas de otros vendedores al vendedor', async () => {
    const { views, servicio } = fabricar();
    const compartida = {
      ...orden,
      total_cents: 3000,
      comision_cents: 360,
      items: [
        ...orden.items,
        {
          ...orden.items[0],
          oferta_id: 'of-2',
          vendedor_id: 'u-v2',
          empresa_id: 'e-2',
        },
      ],
    } as OrderView;
    views.encontrar.mockResolvedValue(compartida);

    const resultado = await servicio.detallePara('o-1', {
      user_id: 'u-v',
      email: 'v@test',
      rol: ROLES.VENDEDOR,
    });

    expect(resultado.items.map((i) => i.oferta_id)).toEqual(['of-1']);
    expect(resultado.total_cents).toBe(1000);
  });

  it('debe recortar una orden a la empresa indicada', () => {
    const { servicio } = fabricar();
    const compartida = {
      ...orden,
      total_cents: 3000,
      items: [
        ...orden.items,
        {
          ...orden.items[0],
          oferta_id: 'of-2',
          empresa_id: 'e-2',
        },
      ],
    } as OrderView;

    const resultado = servicio.paraEmpresa(compartida, 'e-1');

    expect(resultado.items.map((i) => i.oferta_id)).toEqual(['of-1']);
  });

  it('debe ocultar del timeline las líneas no visibles', () => {
    const { servicio } = fabricar();
    const eventos = [
      {
        id: 1,
        tipo: 'OrderCreatedEvent',
        version: 1,
        creado_en: '2026-01-01T00:00:00.000Z',
        payload: {
          order_id: 'o-1',
          items: [
            ...orden.items,
            { ...orden.items[0], oferta_id: 'of-2', precio_unitario_cents: 2000 },
          ],
          total_cents: 3000,
        },
      },
    ];

    const resultado = servicio.timelineVisible(eventos, orden.items);

    expect((resultado[0].payload as { items: { oferta_id: string }[] }).items).toHaveLength(1);
    expect((resultado[0].payload as { total_cents: number }).total_cents).toBe(1000);
  });

  it('debe denegar al empresario una orden de otra empresa o sin empresa activa', async () => {
    const { views, servicio } = fabricar();
    views.encontrar.mockResolvedValue(orden);
    await expect(
      servicio.detallePara('o-1', {
        user_id: 'u-e',
        email: 'e@test',
        rol: ROLES.EMPRESARIO,
        tenant_id: 'e-2',
      }),
    ).rejects.toBeInstanceOf(ForbiddenError);
    await expect(
      servicio.detallePara('o-1', { user_id: 'u-e', email: 'e@test', rol: ROLES.EMPRESARIO }),
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
