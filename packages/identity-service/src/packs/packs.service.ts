import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';

const CODIGO = /^[a-z][a-z0-9_]{1,39}$/;

export interface PackCapacidad {
  capacidad_codigo: string;
  nombre: string;
  modulo: string;
  activo: boolean;
  config: Record<string, unknown>;
}

export interface PackModelo {
  modelo_codigo: string;
  nombre: string;
  descripcion: string;
  activo: boolean;
  config: Record<string, unknown>;
}

export interface ModeloOperacion {
  codigo: string;
  nombre: string;
  descripcion: string;
  config: Record<string, unknown>;
  activo: boolean;
}

export interface VerticalOperacion {
  codigo: string;
  nombre: string;
  modelo_operacion: string;
  activo: boolean;
}

export interface CrearPackInput {
  codigo: string;
  version: number;
  vertical_codigo: string;
  nombre: string;
  descripcion?: string;
  config?: Record<string, unknown>;
  modelos?: { codigo: string; config?: Record<string, unknown>; activo?: boolean }[];
  capacidades?: { codigo: string; config?: Record<string, unknown>; activo?: boolean }[];
}

export interface Pack {
  codigo: string;
  version: number;
  vertical_codigo: string;
  nombre: string;
  descripcion: string;
  config: Record<string, unknown>;
  modelos: PackModelo[];
  capacidades: PackCapacidad[];
}

interface PackRow extends Omit<Pack, 'capacidades' | 'modelos'> {
  capacidad_codigo: string | null;
  capacidad_nombre: string | null;
  capacidad_modulo: string | null;
  capacidad_activo: boolean | null;
  capacidad_config: Record<string, unknown> | null;
  modelos: PackModelo[] | null;
}

@Injectable()
export class PacksService {
  constructor(private readonly pg: PgService) {}

  async listar(verticalCodigo?: string): Promise<Pack[]> {
    const filas = await this.pg.query<PackRow>(
      `SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
              pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
              pc.activo AS capacidad_activo, pc.config AS capacidad_config,
              COALESCE(modelos.items, '[]'::jsonb) AS modelos
       FROM identity.packs p
       LEFT JOIN identity.pack_capacidades pc
         ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
       LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
       LEFT JOIN LATERAL (
         SELECT jsonb_agg(jsonb_build_object(
           'modelo_codigo', pm.modelo_codigo,
           'nombre', mo.nombre,
           'descripcion', mo.descripcion,
           'activo', pm.activo,
           'config', pm.config
         ) ORDER BY pm.modelo_codigo) AS items
           FROM identity.pack_modelos pm
           JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
          WHERE pm.pack_codigo = p.codigo AND pm.pack_version = p.version
       ) modelos ON true
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
              pc.activo AS capacidad_activo, pc.config AS capacidad_config,
              COALESCE(modelos.items, '[]'::jsonb) AS modelos
       FROM identity.packs p
       LEFT JOIN identity.pack_capacidades pc
         ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
       LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
       LEFT JOIN LATERAL (
         SELECT jsonb_agg(jsonb_build_object(
           'modelo_codigo', pm.modelo_codigo,
           'nombre', mo.nombre,
           'descripcion', mo.descripcion,
           'activo', pm.activo,
           'config', pm.config
         ) ORDER BY pm.modelo_codigo) AS items
           FROM identity.pack_modelos pm
           JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
          WHERE pm.pack_codigo = p.codigo AND pm.pack_version = p.version
       ) modelos ON true
       WHERE p.codigo = $1 AND p.version = $2 AND p.activo = true
       ORDER BY pc.capacidad_codigo`,
      [codigo, version],
    );
    const pack = this.agrupar(packs)[0];
    if (!pack) throw new NotFoundError('Pack', `${codigo}@${version}`);
    return pack;
  }

  async listarModelos(): Promise<ModeloOperacion[]> {
    return this.pg.query<ModeloOperacion>(
      `SELECT codigo, nombre, descripcion, config, activo
         FROM identity.modelos_operacion
        WHERE activo = true
        ORDER BY codigo`,
    );
  }

  async listarVerticales(): Promise<VerticalOperacion[]> {
    return this.pg.query<VerticalOperacion>(
      `SELECT codigo, nombre, modelo_operacion, activo
         FROM identity.verticales
        WHERE activo = true
        ORDER BY codigo`,
    );
  }

  async crearModelo(input: Omit<ModeloOperacion, 'activo'> & { activo?: boolean }): Promise<ModeloOperacion> {
    const codigo = this.codigo(input.codigo, 'MODELO_OPERACION_INVALIDO');
    const nombre = input.nombre.trim();
    if (nombre.length < 2 || nombre.length > 120) {
      throw new DomainError('MODELO_OPERACION_INVALIDO', 'El nombre del modelo operativo no es válido.');
    }
    const fila = await this.pg.queryOne<ModeloOperacion>(
      `INSERT INTO identity.modelos_operacion (codigo, nombre, descripcion, config, activo)
       VALUES ($1, $2, $3, $4::jsonb, COALESCE($5, true))
       RETURNING codigo, nombre, descripcion, config, activo`,
      [codigo, nombre, input.descripcion?.trim() ?? '', JSON.stringify(input.config ?? {}), input.activo ?? true],
    );
    if (!fila) throw new ConflictError('No se pudo crear el modelo operativo.');
    return fila;
  }

  async crearVertical(input: Omit<VerticalOperacion, 'activo'> & { activo?: boolean }): Promise<VerticalOperacion> {
    const codigo = this.codigo(input.codigo, 'VERTICAL_INVALIDA');
    const nombre = input.nombre.trim();
    const modelo = input.modelo_operacion.trim();
    if (nombre.length < 2 || nombre.length > 120 || modelo.length < 2 || modelo.length > 160) {
      throw new DomainError('VERTICAL_INVALIDA', 'Los datos de la vertical no son válidos.');
    }
    // El modelo de operación debe existir: evita verticales con modelos libres
    // que luego no pueden componer packs.
    const modeloValido = await this.pg.queryOne<{ codigo: string }>(
      `SELECT codigo FROM identity.modelos_operacion WHERE codigo = $1 AND activo = true`,
      [modelo],
    );
    if (!modeloValido) throw new NotFoundError('Modelo operativo', modelo);
    const fila = await this.pg.queryOne<VerticalOperacion>(
      `INSERT INTO identity.verticales (codigo, nombre, modelo_operacion, activo)
       VALUES ($1, $2, $3, COALESCE($4, true))
       RETURNING codigo, nombre, modelo_operacion, activo`,
      [codigo, nombre, modelo, input.activo ?? true],
    );
    if (!fila) throw new ConflictError('No se pudo crear la vertical.');
    return fila;
  }

  async crear(input: CrearPackInput): Promise<Pack> {
    const codigo = input.codigo.trim().toLowerCase();
    const vertical = this.codigo(input.vertical_codigo, 'VERTICAL_INVALIDA');
    if (!/^pack_[a-z][a-z0-9_]{1,39}$/.test(codigo)) {
      throw new DomainError('PACK_INVALIDO', 'El código del pack debe empezar por pack_.');
    }
    if (!Number.isInteger(input.version) || input.version < 1) {
      throw new DomainError('PACK_INVALIDO', 'La versión del pack debe ser un entero positivo.');
    }
    if (input.nombre.trim().length < 2 || input.nombre.trim().length > 120) {
      throw new DomainError('PACK_INVALIDO', 'El nombre del pack no es válido.');
    }
    await this.pg.transaccion(async (client) => {
      const verticalActiva = await client.query(
        `SELECT 1 FROM identity.verticales WHERE codigo = $1 AND activo = true`,
        [vertical],
      );
      if (!verticalActiva.rowCount) throw new NotFoundError('Vertical', vertical);
      await client.query(
        `INSERT INTO identity.packs (codigo, version, vertical_codigo, nombre, descripcion, config)
         VALUES ($1, $2, $3, $4, $5, $6::jsonb)`,
        [codigo, input.version, vertical, input.nombre.trim(), input.descripcion?.trim() ?? '', JSON.stringify(input.config ?? {})],
      );
      for (const modelo of input.modelos ?? []) {
        const modeloInsertado = await client.query(
          `INSERT INTO identity.pack_modelos (pack_codigo, pack_version, modelo_codigo, config, activo)
           SELECT $1, $2, codigo, $4::jsonb, COALESCE($5, true)
             FROM identity.modelos_operacion
            WHERE codigo = $3 AND activo = true`,
          [codigo, input.version, this.codigo(modelo.codigo, 'MODELO_OPERACION_INVALIDO'), JSON.stringify(modelo.config ?? {}), modelo.activo ?? true],
        );
        if (!modeloInsertado.rowCount) throw new NotFoundError('Modelo operativo', modelo.codigo);
      }
      for (const capacidad of input.capacidades ?? []) {
        const capacidadInsertada = await client.query(
          `INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
           SELECT $1, $2, codigo, $4::jsonb, COALESCE($5, true)
             FROM identity.capacidades
            WHERE codigo = $3 AND activo = true`,
          [codigo, input.version, this.codigo(capacidad.codigo, 'CAPACIDAD_INVALIDA'), JSON.stringify(capacidad.config ?? {}), capacidad.activo ?? true],
        );
        if (!capacidadInsertada.rowCount) throw new NotFoundError('Capacidad', capacidad.codigo);
      }
      // Las capacidades del pack también quedan disponibles en la vertical para
      // que la empresa pueda activarlas/desactivarlas (add-ons) sin código.
      await client.query(
        `INSERT INTO identity.vertical_capacidades AS vc
           (vertical_codigo, capacidad_codigo, config, activo)
         SELECT $1, pc.capacidad_codigo, pc.config, pc.activo
           FROM identity.pack_capacidades pc
          WHERE pc.pack_codigo = $2 AND pc.pack_version = $3
         ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING`,
        [vertical, codigo, input.version],
      );
    });
    return this.obtener(codigo, input.version);
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
          modelos: fila.modelos ?? [],
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

  private codigo(valor: string, codigoError: string): string {
    const codigo = valor.trim().toLowerCase();
    if (!CODIGO.test(codigo)) throw new DomainError(codigoError, 'El código contiene caracteres no permitidos.');
    return codigo;
  }
}
