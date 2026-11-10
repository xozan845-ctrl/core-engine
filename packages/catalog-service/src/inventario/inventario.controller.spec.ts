import 'reflect-metadata';
import { InventarioController } from './inventario.controller';
import type { InventarioService } from './inventario.service';

const lineas = [
  {
    sku: 'SKU-01',
    nombre: 'Teclado',
    stock: 10,
    disponible: 7,
    reservado: 3,
    estado: 'disponible',
    precio_base: '1500.00',
    valor_disponible_cents: 1050000,
    valor_total_cents: 1500000,
  },
];

describe('InventarioController (api/v2/admin/inventario, R-U-10/11)', () => {
  it('debe devolver las lineas de stock que entrega el servicio', async () => {
    const inventario = { listar: vi.fn().mockResolvedValue(lineas) };
    const controller = new InventarioController(inventario as unknown as InventarioService);
    await expect(controller.listar()).resolves.toEqual(lineas);
    expect(inventario.listar).toHaveBeenCalledTimes(1);
  });

  it('debe delegar el alta de proveedores y lotes al servicio', async () => {
    const inventario = {
      crearProveedor: vi.fn().mockResolvedValue({ id: 'prov-1' }),
      crearLote: vi.fn().mockResolvedValue({ id: 'lote-1' }),
      listarLotes: vi.fn().mockResolvedValue([{ id: 'lote-1' }]),
    };
    const controller = new InventarioController(inventario as unknown as InventarioService);

    await expect(
      controller.crearProveedor({ empresa_id: 'e-1', codigo: 'P-1', nombre: 'Proveedor' }),
    ).resolves.toEqual({ id: 'prov-1' });
    await expect(
      controller.crearLote({ empresa_id: 'e-1', item_id: 'i-1', codigo_lote: 'L-1', cantidad_inicial: 2 }),
    ).resolves.toEqual({ id: 'lote-1' });
    await expect(controller.listarLotes('e-1', 'i-1')).resolves.toEqual([{ id: 'lote-1' }]);
    expect(inventario.crearProveedor).toHaveBeenCalled();
    expect(inventario.crearLote).toHaveBeenCalled();
    expect(inventario.listarLotes).toHaveBeenCalledWith('e-1', 'i-1');
  });

  it('debe exigir el rol admin en el endpoint', () => {
    expect(Reflect.getMetadata('roles_requeridos', InventarioController.prototype.listar)).toEqual(['super_admin', 'admin']);
  });
});