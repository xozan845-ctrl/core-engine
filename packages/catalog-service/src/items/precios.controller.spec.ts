import type { Mock } from 'vitest';
import 'reflect-metadata';
import { CAPABILITIES_KEY, CAPACIDADES, ROLES } from '@core/shared';
import { PreciosController } from './precios.controller';
import type { PreciosService } from './precios.service';

const empresario = { user_id: 'u-1', email: 'e@core.local', rol: ROLES.EMPRESARIO, tenant_id: 't-1' };
const admin = { user_id: 'u-2', email: 'a@core.local', rol: ROLES.ADMIN };
const ITEM = '22222222-2222-4222-8222-222222222222';
const LISTA = '33333333-3333-4333-8333-333333333333';
const PRECIO = '44444444-4444-4444-8444-444444444444';

function fabricar(): { servicio: Record<string, Mock>; controller: PreciosController } {
  const servicio = {
    listarListas: vi.fn(),
    crearLista: vi.fn(),
    actualizarLista: vi.fn(),
    listarPrecios: vi.fn(),
    crearPrecio: vi.fn(),
    actualizarPrecio: vi.fn(),
    eliminarPrecio: vi.fn(),
    listarEscalas: vi.fn(),
    actualizarEscala: vi.fn(),
    eliminarEscala: vi.fn(),
    resolver: vi.fn(),
  };
  return { servicio, controller: new PreciosController(servicio as unknown as PreciosService) };
}

describe('PreciosController (catálogo)', () => {
  it('exige la capacidad de catálogo y roles administrativos', () => {
    expect(Reflect.getMetadata(CAPABILITIES_KEY, PreciosController)).toEqual([
      CAPACIDADES.CATALOGO_ITEMS,
    ]);
    expect(Reflect.getMetadata('roles_requeridos', PreciosController)).toEqual([
      ROLES.SUPER_ADMIN,
      ROLES.ADMIN,
      ROLES.EMPRESARIO,
    ]);
  });

  it('usa el tenant del empresario y exige empresa_id al admin global', async () => {
    const { servicio, controller } = fabricar();
    servicio.listarListas.mockResolvedValue([]);
    await expect(controller.listarListas(undefined, empresario as never)).resolves.toEqual([]);
    expect(servicio.listarListas).toHaveBeenCalledWith('t-1');
    expect(() => controller.listarListas(undefined, admin as never)).toThrow(/empresa_id/);
    expect(() => controller.listarListas('otra', empresario as never)).toThrow(
      /No puedes operar otra empresa/,
    );
  });

  it('inyecta el item de la ruta al crear una escala base', async () => {
    const { servicio, controller } = fabricar();
    servicio.crearPrecio.mockResolvedValue({ id: PRECIO });
    await controller.crearEscala(
      ITEM,
      { desde_cantidad: 10, precio_cents: 8000 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crearPrecio).toHaveBeenCalledWith('t-1', null, {
      desde_cantidad: 10,
      precio_cents: 8000,
      item_id: ITEM,
    });
  });


  it('delega el ciclo completo de listas y precios', async () => {
    const { servicio, controller } = fabricar();
    servicio.crearLista.mockResolvedValue({ id: LISTA });
    servicio.actualizarLista.mockResolvedValue({ id: LISTA });
    servicio.listarPrecios.mockResolvedValue([]);
    servicio.actualizarPrecio.mockResolvedValue({ id: PRECIO });
    servicio.eliminarPrecio.mockResolvedValue({ ok: true });
    servicio.listarEscalas.mockResolvedValue([]);

    await controller.crearLista(
      { codigo: 'vip', nombre: 'VIP' } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crearLista).toHaveBeenCalledWith('t-1', { codigo: 'vip', nombre: 'VIP' });

    await controller.actualizarLista(
      LISTA,
      { activo: false } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.actualizarLista).toHaveBeenCalledWith('t-1', LISTA, { activo: false });

    await controller.listarPrecios(LISTA, undefined, empresario as never);
    expect(servicio.listarPrecios).toHaveBeenCalledWith('t-1', LISTA);

    await controller.actualizarPrecio(
      LISTA,
      PRECIO,
      { precio_cents: 9000 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.actualizarPrecio).toHaveBeenCalledWith('t-1', LISTA, PRECIO, {
      precio_cents: 9000,
    });

    await controller.eliminarPrecio(LISTA, PRECIO, undefined, empresario as never);
    expect(servicio.eliminarPrecio).toHaveBeenCalledWith('t-1', LISTA, PRECIO);

    await controller.listarEscalas(ITEM, undefined, empresario as never);
    expect(servicio.listarEscalas).toHaveBeenCalledWith('t-1', ITEM);
  });

  it('permite al admin indicar empresa y rechaza a un empresario ajeno', async () => {
    const { servicio, controller } = fabricar();
    servicio.crearLista.mockResolvedValue({ id: LISTA });
    await controller.crearLista(
      { codigo: 'vip', nombre: 'VIP' } as never,
      'e-9',
      admin as never,
    );
    expect(servicio.crearLista).toHaveBeenCalledWith('e-9', { codigo: 'vip', nombre: 'VIP' });

    expect(() =>
      controller.crearLista({ codigo: 'vip', nombre: 'VIP' } as never, 'otra', empresario as never),
    ).toThrow(/No puedes operar otra empresa/);
    expect(() =>
      controller.resolver(ITEM, '1', undefined, undefined, undefined, {
        ...empresario,
        tenant_id: undefined,
      } as never),
    ).toThrow(/No hay empresa activa/);
  });

  it('delega la resolucion y las operaciones de listas', async () => {
    const { servicio, controller } = fabricar();
    servicio.resolver.mockResolvedValue({ precio_unitario_cents: 7000 });
    servicio.crearPrecio.mockResolvedValue({ id: PRECIO });
    servicio.actualizarEscala.mockResolvedValue({ id: PRECIO });
    servicio.eliminarEscala.mockResolvedValue({ ok: true });

    await controller.resolver(ITEM, '3', 'mayorista', undefined, undefined, empresario as never);
    expect(servicio.resolver).toHaveBeenCalledWith('t-1', ITEM, '3', {
      lista_codigo: 'mayorista',
      segmento_codigo: undefined,
    });

    await controller.crearPrecio(
      LISTA,
      { item_id: ITEM, precio_cents: 9000 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.crearPrecio).toHaveBeenCalledWith('t-1', LISTA, {
      item_id: ITEM,
      precio_cents: 9000,
    });

    await controller.actualizarEscala(
      ITEM,
      PRECIO,
      { precio_cents: 7000 } as never,
      undefined,
      empresario as never,
    );
    expect(servicio.actualizarEscala).toHaveBeenCalledWith('t-1', ITEM, PRECIO, {
      precio_cents: 7000,
    });

    await expect(
      controller.eliminarEscala(ITEM, PRECIO, undefined, empresario as never),
    ).resolves.toEqual({ ok: true });
    expect(servicio.eliminarEscala).toHaveBeenCalledWith('t-1', ITEM, PRECIO);
  });
});
