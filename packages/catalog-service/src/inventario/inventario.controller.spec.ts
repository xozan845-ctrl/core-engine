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

describe('InventarioController (api/v1/admin/inventario, R-U-10/11)', () => {
  it('debe devolver las lineas de stock que entrega el servicio', async () => {
    const inventario = { listar: jest.fn().mockResolvedValue(lineas) };
    const controller = new InventarioController(inventario as unknown as InventarioService);
    await expect(controller.listar()).resolves.toEqual(lineas);
    expect(inventario.listar).toHaveBeenCalledTimes(1);
  });

  it('debe exigir el rol admin en el endpoint', () => {
    expect(Reflect.getMetadata('roles_requeridos', InventarioController.prototype.listar)).toEqual(['admin']);
  });
});