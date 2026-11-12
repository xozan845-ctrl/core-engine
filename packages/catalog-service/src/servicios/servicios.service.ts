import { Injectable, Optional } from '@nestjs/common';
import {
  ConflictError,
  DomainError,
  Money,
  NotFoundError,
  Pagina,
  PgService,
  crearPagina,
  parsearPaginacion,
} from '@core/shared';
import { ProductosService } from '../productos/productos.service';
import { ItemsService } from '../items/items.service';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const CODIGO = /^[A-Z0-9-]{2,32}$/;
const MAX_DURACION = 1440;

export interface Servicio {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  marca: string;
  codigo_barras: string;
  precio: string;
  costo: string;
  unidad: string;
  duracion_minutos: number;
  estado: 'activo' | 'inactivo';
  atributos: Record<string, unknown>;
  creado_en: string;
  actualizado_en: string;
}

interface ServicioRow {
  id: string;
  empresa_id: string;
  codigo: string;
  nombre: string;
  descripcion: string;
  categoria: string;
  marca: string;
  codigo_barras: string;
  precio_cents: number;
  costo_cents: number;
  unidad: string;
  duracion_minutos: number;
  estado: string;
  atributos: Record<string, unknown> | null;
  creado_en: string;
  actualizado_en: string;
}

export interface CrearServicioInput {
  codigo: string;
  nombre: string;
  descripcion?: string;
  categoria?: string;
  marca?: string;
  codigo_barras?: string;
  precio: string | number;
  costo?: string | number;
  unidad?: string;
  duracion_minutos: number;
  estado?: 'activo' | 'inactivo';
  atributos?: Record<string, unknown>;
}

export interface ActualizarServicioInput {
  nombre?: string;
  descripcion?: string;
  categoria?: string;
  marca?: string;
  codigo_barras?: string;
  precio?: string | number;
  costo?: string | number;
  unidad?: string;
  duracion_minutos?: number;
  estado?: 'activo' | 'inactivo';
  atributos?: Record<string, unknown>;
}

const COLUMNAS_SERVICIO = `i.id, i.empresa_id, i.codigo, i.nombre, i.descripcion,
  i.categoria, i.marca, i.codigo_barras, i.precio_cents, i.costo_cents, i.unidad,
  i.duration_minutes AS duracion_minutos, i.atributos, i.estado, i.creado_en,
  i.actualizado_en`;

/**
 * Servicios de la empresa (R-PS-02). Es un recurso propio del catálogo, no un
 * producto sin stock: tiene duración, precio/costo y baja lógica. Desde la
 * Fase 3 vive en `catalog.items` con `tipo='servicio'`.
 */
@Injectable()
export class ServiciosService {
  constructor(
    private readonly pg: PgService,
    private readonly productos: ProductosService,
    @Optional() private readonly items?: ItemsService,
  ) {}

  async listar(filtros: {
    empresa_id?: unknown;
    pagina?: unknown;
    limite?: unknown;
    q?: unknown;
    estado?: unknown;
    categoria?: unknown;
  }): Promise<Pagina<Servicio>> {
    const empresaId = this.exigirEmpresa(filtros.empresa_id);
    const { pagina, limite } = parsearPaginacion(filtros as Record<string, unknown>);
    const params: unknown[] = [empresaId];
    const condiciones: string[] = [
      'i.empresa_id = $1',
      "i.tipo = 'servicio'",
      'i.eliminado_en IS NULL',
    ];
    if (filtros.q) {
      params.push(`%${String(filtros.q).toLowerCase()}%`);
      condiciones.push(
        `(LOWER(i.nombre) LIKE $${params.length} OR LOWER(i.codigo) LIKE $${params.length}
          OR LOWER(i.marca) LIKE $${params.length} OR LOWER(i.codigo_barras) LIKE $${params.length})`,
      );
    }
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`i.estado = $${params.length}`);
    }
    if (filtros.categoria) {
      params.push(String(filtros.categoria));
      condiciones.push(`LOWER(i.categoria) = LOWER($${params.length})`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM catalog.items i ${where}`, params)
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<ServicioRow>(
      `SELECT ${COLUMNAS_SERVICIO}
         FROM catalog.items i ${where}
        ORDER BY i.creado_en DESC
        LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, (pagina - 1) * limite],
    );
    return crearPagina(filas.map((fila) => this.serializar(fila)), total, { pagina, limite });
  }

  async encontrarPorId(id: string, empresaId: string): Promise<Servicio | null> {
    if (!UUID.test(id)) return null;
    const fila = await this.pg.queryOne<ServicioRow>(
      `SELECT ${COLUMNAS_SERVICIO}
         FROM catalog.items i
        WHERE i.id = $1 AND i.empresa_id = $2 AND i.tipo = 'servicio'
          AND i.eliminado_en IS NULL`,
      [id, empresaId],
    );
    return fila ? this.serializar(fila) : null;
  }

  /** Escaneo: código de barras o código interno, solo servicios activos. */
  async encontrarPorCodigo(codigo: string, empresaId: string): Promise<Servicio | null> {
    const valor = codigo.trim();
    if (!valor) return null;
    const fila = await this.pg.queryOne<ServicioRow>(
      `SELECT ${COLUMNAS_SERVICIO}
         FROM catalog.items i
        WHERE i.empresa_id = $1 AND i.tipo = 'servicio' AND i.eliminado_en IS NULL
          AND i.estado = 'activo'
          AND (i.codigo_barras = $2 OR upper(i.codigo) = upper($2))
        LIMIT 1`,
      [empresaId, valor],
    );
    return fila ? this.serializar(fila) : null;
  }

  async crear(datos: CrearServicioInput, empresaId: string): Promise<Servicio> {
    await this.productos.validarEmpresaCatalogo(empresaId);
    const codigo = this.normalizarCodigo(datos.codigo);
    const nombre = this.validarNombre(datos.nombre);
    const precio = this.validarPrecio(datos.precio);
    const costo = this.validarPrecio(datos.costo ?? '0');
    const duracion = this.validarDuracion(datos.duracion_minutos);
    const atributos = datos.atributos ?? {};
    await this.items?.validarAtributos(empresaId, 'servicio', datos.categoria ?? 'general', atributos);
    const codigoBarras = (datos.codigo_barras ?? '').trim();
    await this.validarCodigoUnico(empresaId, codigo);
    await this.validarBarcodeUnico(empresaId, codigoBarras);
    const fila = await this.pg.queryOne<ServicioRow>(
      `INSERT INTO catalog.items
         (empresa_id, tipo, codigo, nombre, descripcion, categoria, marca, codigo_barras,
          precio_cents, costo_cents, unidad, duration_minutes, atributos, estado,
          track_inventory)
       VALUES ($1, 'servicio', $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13,
               false)
       RETURNING id, empresa_id, codigo, nombre, descripcion, categoria, marca,
                 codigo_barras, precio_cents, costo_cents, unidad,
                 duration_minutes AS duracion_minutos, atributos, estado, creado_en,
                 actualizado_en`,
      [
        empresaId,
        codigo,
        nombre,
        (datos.descripcion ?? '').trim(),
        (datos.categoria ?? '').trim(),
        (datos.marca ?? '').trim(),
        codigoBarras,
        precio.centavos,
        costo.centavos,
        (datos.unidad ?? 'Hora (h)').trim() || 'Hora (h)',
        duracion,
        JSON.stringify(atributos),
        datos.estado === 'inactivo' ? 'inactivo' : 'activo',
      ],
    );
    if (!fila) throw new DomainError('SERVICIO_NO_CREADO', 'No se pudo crear el servicio.');
    return this.serializar(fila);
  }

  async actualizar(
    id: string,
    cambios: ActualizarServicioInput,
    empresaId: string,
  ): Promise<Servicio> {
    const actual = await this.encontrarPorId(id, empresaId);
    if (!actual) throw new NotFoundError('Servicio', id);
    const precio =
      cambios.precio === undefined ? Money.parsear(actual.precio) : this.validarPrecio(cambios.precio);
    const costo =
      cambios.costo === undefined ? Money.parsear(actual.costo) : this.validarPrecio(cambios.costo);
    const duracion =
      cambios.duracion_minutos === undefined
        ? actual.duracion_minutos
        : this.validarDuracion(cambios.duracion_minutos);
    const nombre = cambios.nombre === undefined ? actual.nombre : this.validarNombre(cambios.nombre);
    const codigoBarras =
      cambios.codigo_barras === undefined ? actual.codigo_barras : cambios.codigo_barras.trim();
    const atributos = cambios.atributos === undefined ? actual.atributos : cambios.atributos;
    if (cambios.atributos !== undefined) {
      await this.items?.validarAtributos(empresaId, 'servicio', cambios.categoria ?? actual.categoria, atributos);
    }
    if (codigoBarras !== actual.codigo_barras) {
      await this.validarBarcodeUnico(empresaId, codigoBarras, id);
    }
    const fila = await this.pg.queryOne<ServicioRow>(
      `UPDATE catalog.items
          SET nombre = $3, descripcion = $4, categoria = $5, marca = $6,
              codigo_barras = $7, precio_cents = $8, costo_cents = $9, unidad = $10,
              duration_minutes = $11, atributos = $12::jsonb, estado = $13,
              actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND tipo = 'servicio' AND eliminado_en IS NULL
        RETURNING id, empresa_id, codigo, nombre, descripcion, categoria, marca,
                  codigo_barras, precio_cents, costo_cents, unidad,
                  duration_minutes AS duracion_minutos, atributos, estado, creado_en,
                  actualizado_en`,
      [
        id,
        empresaId,
        nombre,
        cambios.descripcion === undefined ? actual.descripcion : cambios.descripcion.trim(),
        cambios.categoria === undefined ? actual.categoria : cambios.categoria.trim(),
        cambios.marca === undefined ? actual.marca : cambios.marca.trim(),
        codigoBarras,
        precio.centavos,
        costo.centavos,
        cambios.unidad === undefined ? actual.unidad : cambios.unidad.trim() || actual.unidad,
        duracion,
        JSON.stringify(atributos),
        cambios.estado ?? actual.estado,
      ],
    );
    if (!fila) throw new NotFoundError('Servicio', id);
    return this.serializar(fila);
  }

  /** Baja lógica: conserva el registro y lo marca inactivo (R-DB-5). */
  async eliminar(id: string, empresaId: string): Promise<void> {
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE catalog.items
          SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND tipo = 'servicio' AND eliminado_en IS NULL
        RETURNING id`,
      [id, empresaId],
    );
    if (!fila) throw new NotFoundError('Servicio', id);
  }

  private serializar(fila: ServicioRow): Servicio {
    return {
      id: fila.id,
      empresa_id: fila.empresa_id,
      codigo: fila.codigo,
      nombre: fila.nombre,
      descripcion: fila.descripcion,
      categoria: fila.categoria,
      marca: fila.marca,
      codigo_barras: fila.codigo_barras,
      precio: Money.desdeCentavos(fila.precio_cents).string(),
      costo: Money.desdeCentavos(fila.costo_cents).string(),
      unidad: fila.unidad,
      duracion_minutos: fila.duracion_minutos,
      atributos: fila.atributos ?? {},
      estado: fila.estado === 'inactivo' ? 'inactivo' : 'activo',
      creado_en: fila.creado_en,
      actualizado_en: fila.actualizado_en,
    };
  }

  private exigirEmpresa(valor: unknown): string {
    const empresaId = typeof valor === 'string' ? valor.trim() : '';
    if (!empresaId) {
      throw new DomainError('EMPRESA_REQUERIDA', 'Los servicios se consultan por empresa.');
    }
    return empresaId;
  }

  private normalizarCodigo(valor: string): string {
    const codigo = String(valor).trim().toUpperCase();
    if (!CODIGO.test(codigo)) {
      throw new DomainError(
        'CODIGO_INVALIDO',
        'El código del servicio solo admite letras, números y guiones (2-32).',
      );
    }
    return codigo;
  }

  private validarNombre(valor: string): string {
    const nombre = String(valor).trim();
    if (nombre.length < 2 || nombre.length > 200) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre del servicio debe tener entre 2 y 200 caracteres.');
    }
    return nombre;
  }

  private validarPrecio(valor: string | number): Money {
    return Money.parsear(valor);
  }

  private validarDuracion(valor: number): number {
    if (!Number.isInteger(valor) || valor < 1 || valor > MAX_DURACION) {
      throw new DomainError(
        'DURACION_INVALIDA',
        `La duración debe ser un entero entre 1 y ${MAX_DURACION} minutos.`,
      );
    }
    return valor;
  }

  private async validarCodigoUnico(empresaId: string, codigo: string): Promise<void> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM catalog.items
        WHERE empresa_id = $1 AND tipo = 'servicio' AND upper(codigo) = $2
          AND eliminado_en IS NULL`,
      [empresaId, codigo],
    );
    if (fila) throw new ConflictError(`Ya existe un servicio con el código ${codigo}.`);
  }

  private async validarBarcodeUnico(
    empresaId: string,
    codigoBarras: string,
    excluirId?: string,
  ): Promise<void> {
    if (!codigoBarras) return;
    const params: unknown[] = [empresaId, codigoBarras];
    let where = 'empresa_id = $1 AND codigo_barras = $2 AND eliminado_en IS NULL';
    if (excluirId) {
      params.push(excluirId);
      where += ` AND id <> $${params.length}`;
    }
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM catalog.items WHERE ${where}`,
      params,
    );
    if (fila) throw new ConflictError(`El código de barras ${codigoBarras} ya está registrado.`);
  }
}
