import { NotFoundError } from '@core/shared';
import { PacksService } from './packs.service';

const fila = {
  codigo: 'pack_belleza',
  version: 1,
  vertical_codigo: 'belleza',
  nombre: 'Belleza',
  descripcion: 'Servicios agendados',
  config: { modelo: 'servicios_agendados' },
  capacidad_codigo: 'agenda_recursos',
  capacidad_nombre: 'Agenda y recursos',
  capacidad_modulo: 'scheduling',
  capacidad_activo: true,
  capacidad_config: {},
};

describe('PacksService', () => {
  it('debe agrupar las capacidades de los packs cuando se listan por vertical', async () => {
    const pg = { query: vi.fn().mockResolvedValue([fila, { ...fila, capacidad_codigo: 'facturacion', capacidad_nombre: 'Facturacion', capacidad_modulo: 'finance' }]) };
    const servicio = new PacksService(pg as never);

    await expect(servicio.listar('belleza')).resolves.toEqual([{
      codigo: 'pack_belleza', version: 1, vertical_codigo: 'belleza', nombre: 'Belleza',
      descripcion: 'Servicios agendados', config: { modelo: 'servicios_agendados' },
      capacidades: [
        { capacidad_codigo: 'agenda_recursos', nombre: 'Agenda y recursos', modulo: 'scheduling', activo: true, config: {} },
        { capacidad_codigo: 'facturacion', nombre: 'Facturacion', modulo: 'finance', activo: true, config: {} },
      ],
    }]);
    expect(pg.query).toHaveBeenCalledWith(expect.stringContaining('p.vertical_codigo = $1'), ['belleza']);
  });

  it('debe devolver un pack versionado', async () => {
    const pg = { query: vi.fn().mockResolvedValue([fila]) };
    await expect(new PacksService(pg as never).obtener('pack_belleza', 1)).resolves.toMatchObject({ codigo: 'pack_belleza', version: 1 });
  });

  it('debe lanzar NotFoundError cuando el pack no existe', async () => {
    const pg = { query: vi.fn().mockResolvedValue([]) };
    await expect(new PacksService(pg as never).obtener('pack_no', 1)).rejects.toBeInstanceOf(NotFoundError);
  });
});
