import { Injectable } from '@nestjs/common';
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
}

/**
 * Servicios de la empresa (R-PS-02). Es un recurso propio del catálogo, no un
 * producto sin stock: tiene duración, precio/costo y baja lógica por empresa.
 */
@Injectable()
export class ServiciosService {
  constructor(
    private readonly pg: PgService,
    private readonly productos: ProductosService,
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
    const condiciones: string[] = ['empresa_id = $1', 'eliminado_en IS NULL'];
    if (filtros.q) {
      params.push(`%${String(filtros.q).toLowerCase()}%`);
      condiciones.push(
        `(LOWER(nombre) LIKE $${params.length} OR LOWER(codigo) LIKE $${params.length}
          OR LOWER(marca) LIKE $${params.length} OR LOWER(codigo_barras) LIKE $${params.length})`,
      );
    }
    if (filtros.estado) {
      params.push(String(filtros.estado));
      condiciones.push(`estado = $${params.length}`);
    }
    if (filtros.categoria) {
      params.push(String(filtros.categoria));
      condiciones.push(`LOWER(categoria) = LOWER($${params.length})`);
    }
    const where = `WHERE ${condiciones.join(' AND ')}`;
    const total = await this.pg
      .queryOne<{ n: number }>(`SELECT COUNT(*)::int AS n FROM catalog.servicios ${where}`, params)
      .then((r) => r?.n ?? 0);
    const filas = await this.pg.query<ServicioRow>(
      `SELECT id, empresa_id, codigo, nombre, descripcion, categoria, marca,
              codigo_barras, precio_cents, costo_cents, unidad, duracion_minutos,
              estado, creado_en, actualizado_en
         FROM catalog.servicios ${where}
        ORDER BY creado_en DESC
        LIMIT $${params.length + 1} OFFSET $${params.length + 2}`,
      [...params, limite, (pagina - 1) * limite],
    );
    return crearPagina(filas.map((fila) => this.serializar(fila)), total, { pagina, limite });
  }

  async encontrarPorId(id: string, empresaId: string): Promise<Servicio | null> {
    if (!UUID.test(id)) return null;
    const fila = await this.pg.queryOne<ServicioRow>(
      `SELECT id, empresa_id, codigo, nombre, descripcion, categoria, marca,
              codigo_barras, precio_cents, costo_cents, unidad, duracion_minutos,
              estado, creado_en, actualizado_en
         FROM catalog.servicios
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [id, empresaId],
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
    const codigoBarras = (datos.codigo_barras ?? '').trim();
    await this.validarCodigoUnico(empresaId, codigo);
    await this.validarBarcodeUnico(empresaId, codigoBarras);
    const fila = await this.pg.queryOne<ServicioRow>(
      `INSERT INTO catalog.servicios
         (empresa_id, codigo, nombre, descripcion, categoria, marca, codigo_barras,
          precio_cents, costo_cents, unidad, duracion_minutos, estado)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
       RETURNING id, empresa_id, codigo, nombre, descripcion, categoria, marca,
                 codigo_barras, precio_cents, costo_cents, unidad, duracion_minutos,
                 estado, creado_en, actualizado_en`,
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
    if (codigoBarras !== actual.codigo_barras) {
      await this.validarBarcodeUnico(empresaId, codigoBarras, id);
    }
    const fila = await this.pg.queryOne<ServicioRow>(
      `UPDATE catalog.servicios
          SET nombre = $3, descripcion = $4, categoria = $5, marca = $6,
              codigo_barras = $7, precio_cents = $8, costo_cents = $9, unidad = $10,
              duracion_minutos = $11, estado = $12, actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL
        RETURNING id, empresa_id, codigo, nombre, descripcion, categoria, marca,
                  codigo_barras, precio_cents, costo_cents, unidad, duracion_minutos,
                  estado, creado_en, actualizado_en`,
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
        cambios.estado ?? actual.estado,
      ],
    );
    if (!fila) throw new NotFoundError('Servicio', id);
    return this.serializar(fila);
  }

  /** Baja lógica: conserva el registro y lo marca inactivo (R-DB-5). */
  async eliminar(id: string, empresaId: string): Promise<void> {
    const fila = await this.pg.queryOne<{ id: string }>(
      `UPDATE catalog.servicios
          SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL
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
    const precio = Money.parsear(valor);
    return precio;
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
      `SELECT 1 FROM catalog.servicios
        WHERE empresa_id = $1 AND upper(codigo) = $2 AND eliminado_en IS NULL`,
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
    const fila = await this.pg.queryOne(`SELECT 1 FROM catalog.servicios WHERE ${where}`, params);
    if (fila) throw new ConflictError(`El código de barras ${codigoBarras} ya está registrado.`);
  }
}
