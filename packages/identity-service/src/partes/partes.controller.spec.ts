import { describe, expect, it, vi } from 'vitest';
import { PartesController } from './partes.controller';

const EMPRESA = '11111111-2222-4333-8444-555555555555';
const PARTE = '22222222-3333-4444-8555-666666666666';
const CONTACTO = '66666666-7777-4888-8999-aaaaaaaaaaaa';
const DIRECCION = '77777777-8888-4999-8aaa-bbbbbbbbbbbb';
const RELACION = '88888888-9999-4aaa-8bbb-cccccccccccc';
const actor = { user_id: '44444444-5555-4666-8777-888888888888', rol: 'empresario' };

function crear(): { controller: PartesController; servicio: Record<string, ReturnType<typeof vi.fn>> } {
  const nombres = [
    'listar', 'crear', 'obtener', 'actualizar', 'eliminar',
    'listarContactos', 'crearContacto', 'actualizarContacto', 'eliminarContacto',
    'listarDirecciones', 'crearDireccion', 'actualizarDireccion', 'eliminarDireccion',
    'listarRelaciones', 'crearRelacion', 'eliminarRelacion',
  ];
  const servicio = Object.fromEntries(nombres.map((nombre) => [nombre, vi.fn().mockResolvedValue({ ok: true })]));
  return { controller: new PartesController(servicio as never), servicio };
}

describe('PartesController', () => {
  it('delega toda la superficie de partes al servicio con los parámetros del contrato', async () => {
    const { controller, servicio } = crear();

    await controller.listar(EMPRESA, 'ana', 'persona', '25', actor as never);
    await controller.crear(EMPRESA, { tipo: 'persona', nombre: 'Ana' } as never, actor as never);
    await controller.obtener(EMPRESA, PARTE, actor as never);
    await controller.actualizar(EMPRESA, PARTE, { nombre: 'Ana María' } as never, actor as never);
    await controller.eliminar(EMPRESA, PARTE, actor as never);
    await controller.listarContactos(EMPRESA, PARTE, actor as never);
    await controller.crearContacto(EMPRESA, PARTE, { tipo: 'email', valor: 'ana@example.test' } as never, actor as never);
    await controller.actualizarContacto(EMPRESA, PARTE, CONTACTO, { valor: 'nuevo@example.test' } as never, actor as never);
    await controller.eliminarContacto(EMPRESA, PARTE, CONTACTO, actor as never);
    await controller.listarDirecciones(EMPRESA, PARTE, actor as never);
    await controller.crearDireccion(EMPRESA, PARTE, { tipo: 'entrega', linea_1: 'Calle 1' } as never, actor as never);
    await controller.actualizarDireccion(EMPRESA, PARTE, DIRECCION, { ciudad: 'Managua' } as never, actor as never);
    await controller.eliminarDireccion(EMPRESA, PARTE, DIRECCION, actor as never);
    await controller.listarRelaciones(EMPRESA, PARTE, actor as never);
    await controller.crearRelacion(EMPRESA, PARTE, { parte_destino_id: DIRECCION, tipo: 'cliente' } as never, actor as never);
    await controller.eliminarRelacion(EMPRESA, PARTE, RELACION, actor as never);

    expect(servicio.listar).toHaveBeenCalledWith(EMPRESA, { q: 'ana', tipo: 'persona', limite: 25 }, actor);
    expect(servicio.crearRelacion).toHaveBeenCalledWith(EMPRESA, PARTE, DIRECCION, 'cliente', undefined, actor);
    expect(servicio.eliminarRelacion).toHaveBeenCalledWith(EMPRESA, PARTE, RELACION, actor);
    expect(Object.values(servicio).every((mock) => mock.mock.calls.length === 1)).toBe(true);
  });

  it('convierte un límite ausente a undefined al listar', async () => {
    const { controller, servicio } = crear();
    await controller.listar(EMPRESA, undefined, undefined, undefined, actor as never);
    expect(servicio.listar).toHaveBeenCalledWith(EMPRESA, { q: undefined, tipo: undefined, limite: undefined }, actor);
  });
});
