import { Injectable } from '@nestjs/common';
import { PgService, NotFoundError } from '@core/shared';

export interface PackCapacidad {
  capacidad_codigo: string;
  nombre: string;
  modulo: string;
  activo: boolean;
  config: Record<string, unknown>;
}

export interface Pack {
  codigo: string;
  version: number;
  vertical_codigo: string;
  nombre: string;
  descripcion: string;
  config: Record<string, unknown>;
  capacidades: PackCapacidad[];
}

interface PackRow extends Omit<Pack, 'capacidades'> {
  capacidad_codigo: string | null;
  capacidad_nombre: string | null;
  capacidad_modulo: string | null;
  capacidad_activo: boolean | null;
  capacidad_config: Record<string, unknown> | null;
}

@Injectable()
export class PacksService {
  constructor(private readonly pg: PgService) {}

  async listar(verticalCodigo?: string): Promise<Pack[]> {
    const filas = await this.pg.query<PackRow>(
      `SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
              pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
              pc.activo AS capacidad_activo, pc.config AS capacidad_config
       FROM identity.packs p
       LEFT JOIN identity.pack_capacidades pc
         ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
       LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
       WHERE p.activo = true
         AND ($1::text IS NULL OR p.vertical_codigo = $1)
       ORDER BY p.vertical_codigo, p.codigo, pc.capacidad_codigo`,
      [verticalCodigo ?? null],
    );
    return this.agrupar(filas);
  }

  async obtener(codigo: string, version = 1): Promise<Pack> {
    const packs = await this.pg.query<PackRow>(
      `SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
              pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
              pc.activo AS capacidad_activo, pc.config AS capacidad_config
       FROM identity.packs p
       LEFT JOIN identity.pack_capacidades pc
         ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
       LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
       WHERE p.codigo = $1 AND p.version = $2 AND p.activo = true
       ORDER BY pc.capacidad_codigo`,
      [codigo, version],
    );
    const pack = this.agrupar(packs)[0];
    if (!pack) throw new NotFoundError('Pack', `${codigo}@${version}`);
    return pack;
  }

  private agrupar(filas: PackRow[]): Pack[] {
    const porCodigo = new Map<string, Pack>();
    for (const fila of filas) {
      const clave = `${fila.codigo}@${fila.version}`;
      let pack = porCodigo.get(clave);
      if (!pack) {
        pack = {
          codigo: fila.codigo,
          version: fila.version,
          vertical_codigo: fila.vertical_codigo,
          nombre: fila.nombre,
          descripcion: fila.descripcion,
          config: fila.config,
          capacidades: [],
        };
        porCodigo.set(clave, pack);
      }
      if (fila.capacidad_codigo) {
        pack.capacidades.push({
          capacidad_codigo: fila.capacidad_codigo,
          nombre: fila.capacidad_nombre ?? fila.capacidad_codigo,
          modulo: fila.capacidad_modulo ?? 'shared',
          activo: fila.capacidad_activo ?? false,
          config: fila.capacidad_config ?? {},
        });
      }
    }
    return [...porCodigo.values()];
  }
}
