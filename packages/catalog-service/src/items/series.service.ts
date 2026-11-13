import { Injectable } from '@nestjs/common';
import { ConflictError, DomainError, NotFoundError, PgService } from '@core/shared';

/**
 * Serial/IMEI por unidad (Fase P1.3, ADR-0001).
 *
 * Un item con `track_serial` mantiene una fila por unidad fisica. La venta la
 * marca como vendida y calcula `garantia_hasta` con los meses del item; la
 * devolucion la marca como devuelta. El stock agregado sigue siendo la
 * autoridad de disponibilidad.
 */

export type EstadoSerie = 'disponible' | 'reservado' | 'vendido' | 'devuelto' | 'baja';
export const ESTADOS_SERIE: EstadoSerie[] = [
  'disponible',
  'reservado',
  'vendido',
  'devuelto',
  'baja',
];
/** Estados que un operador puede fijar manualmente. */
const ESTADOS_MANUALES: EstadoSerie[] = ['disponible', 'baja'];

export interface ItemSerie {
  id: string;
  empresa_id: string;
  item_id: string;
  ubicacion_id: string | null;
  serial: string;
  imei: string;
  estado: EstadoSerie;
  order_id: string | null;
  vendido_en: string | null;
  garantia_hasta: string | null;
  devuelto_en: string | null;
  metadata: Record<string, unknown>;
  creado_en: string;
  actualizado_en: string;
}

export interface ItemSerieDetalle extends ItemSerie {
  sku: string;
  nombre_item: string;
  garantia_meses: number | null;
  garantia_vigente: boolean;
}

export interface CrearItemSerieInput {
  serial: string;
  imei?: string;
  ubicacion_id?: string | null;
  metadata?: Record<string, unknown>;
}

export interface ActualizarItemSerieInput {
  serial?: string;
  imei?: string;
  ubicacion_id?: string | null;
  estado?: EstadoSerie;
  metadata?: Record<string, unknown>;
}

interface ItemSerializado {
  id: string;
  tipo: string;
  track_inventory: boolean;
  track_serial: boolean;
  codigo: string;
  nombre: string;
  garantia_meses: number | null;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const SERIAL = /^[A-Za-z0-9._-]{1,120}$/;
const IMEI = /^[0-9]{0,20}$/;

const COLUMNAS = `id, empresa_id, item_id, ubicacion_id, serial, imei, estado,
  order_id, vendido_en, garantia_hasta, devuelto_en, metadata, creado_en,
  actualizado_en`;

@Injectable()
export class SeriesService {
  constructor(private readonly pg: PgService) {}

  async listar(empresaId: string, itemId: string): Promise<ItemSerie[]> {
    await this.verificarItem(empresaId, itemId);
    return this.pg.query<ItemSerie>(
      `SELECT ${COLUMNAS}
         FROM catalog.item_series
        WHERE empresa_id = $1 AND item_id = $2
        ORDER BY estado, creado_en`,
      [empresaId, itemId],
    );
  }

  async crear(empresaId: string, itemId: string, input: CrearItemSerieInput): Promise<ItemSerie> {
    const item = await this.verificarItem(empresaId, itemId);
    if (!item.track_serial) {
      throw new DomainError(
        'SERIALIZACION_NO_ACTIVA',
        'Activa la serializacion del item antes de registrar unidades.',
      );
    }
    const serial = this.validarSerial(input.serial);
    const imei = this.validarImei(input.imei);
    const ubicacionId = await this.validarUbicacion(empresaId, input.ubicacion_id);
    try {
      const fila = await this.pg.queryOne<ItemSerie>(
        `INSERT INTO catalog.item_series
           (empresa_id, item_id, ubicacion_id, serial, imei, metadata)
         VALUES ($1, $2, $3, $4, $5, $6::jsonb)
         RETURNING ${COLUMNAS}`,
        [empresaId, itemId, ubicacionId, serial, imei, JSON.stringify(input.metadata ?? {})],
      );
      if (!fila) throw new DomainError('SERIE_NO_CREADA', 'No se pudo registrar la unidad.');
      return fila;
    } catch (error) {
      if (this.esConflicto(error)) {
        throw new ConflictError('El serial o IMEI ya existe en esta empresa.');
      }
      throw error;
    }
  }

  async actualizar(
    empresaId: string,
    serieId: string,
    input: ActualizarItemSerieInput,
  ): Promise<ItemSerie> {
    const actual = await this.buscar(empresaId, serieId);
    const serial = input.serial === undefined ? actual.serial : this.validarSerial(input.serial);
    const imei = input.imei === undefined ? actual.imei : this.validarImei(input.imei);
    const ubicacionId = input.ubicacion_id === undefined
      ? actual.ubicacion_id
      : await this.validarUbicacion(empresaId, input.ubicacion_id);
    const estado = input.estado === undefined ? actual.estado : this.validarEstadoManual(input.estado);
    const metadata = input.metadata === undefined ? actual.metadata : input.metadata;
    try {
      const fila = await this.pg.queryOne<ItemSerie>(
        `UPDATE catalog.item_series
            SET serial = $1, imei = $2, ubicacion_id = $3, estado = $4,
                metadata = $5::jsonb,
                devuelto_en = CASE WHEN $4 = 'disponible' THEN NULL ELSE devuelto_en END,
                actualizado_en = NOW()
          WHERE id = $6 AND empresa_id = $7
          RETURNING ${COLUMNAS}`,
        [serial, imei, ubicacionId, estado, JSON.stringify(metadata), serieId, empresaId],
      );
      if (!fila) throw new NotFoundError('Serie', serieId);
      return fila;
    } catch (error) {
      if (this.esConflicto(error)) {
        throw new ConflictError('El serial o IMEI ya existe en esta empresa.');
      }
      throw error;
    }
  }

  async buscarPorSerial(empresaId: string, serialValor: unknown): Promise<ItemSerieDetalle> {
    const serial = this.validarSerial(serialValor);
    const fila = await this.pg.queryOne<ItemSerieDetalle>(
      `SELECT s.id, s.empresa_id, s.item_id, s.ubicacion_id, s.serial, s.imei,
              s.estado, s.order_id, s.vendido_en, s.garantia_hasta, s.devuelto_en,
              s.metadata, s.creado_en, s.actualizado_en,
              i.codigo AS sku, i.nombre AS nombre_item, i.garantia_meses,
              (s.garantia_hasta IS NOT NULL AND s.garantia_hasta >= CURRENT_DATE) AS garantia_vigente
         FROM catalog.item_series s
         JOIN catalog.items i ON i.id = s.item_id
        WHERE s.empresa_id = $1 AND lower(s.serial) = lower($2)`,
      [empresaId, serial],
    );
    if (!fila) throw new NotFoundError('Serie', serial);
    return fila;
  }

  async habilitarSerializacion(
    empresaId: string,
    itemId: string,
    activo: boolean,
  ): Promise<{ item_id: string; track_serial: boolean }> {
    const item = await this.verificarItem(empresaId, itemId);
    if (activo && (item.tipo !== 'producto' || !item.track_inventory)) {
      throw new DomainError(
        'SERIALIZACION_NO_APLICA',
        'Solo los productos con inventario pueden serializarse.',
      );
    }
    const fila = await this.pg.queryOne<{ item_id: string; track_serial: boolean }>(
      `UPDATE catalog.items
          SET track_serial = $1, actualizado_en = NOW()
        WHERE id = $2 AND empresa_id = $3
        RETURNING id AS item_id, track_serial`,
      [activo, itemId, empresaId],
    );
    if (!fila) throw new NotFoundError('Item', itemId);
    return fila;
  }

  private async buscar(empresaId: string, serieId: string): Promise<ItemSerie> {
    this.validarUuid(serieId, 'serie');
    const fila = await this.pg.queryOne<ItemSerie>(
      `SELECT ${COLUMNAS}
         FROM catalog.item_series
        WHERE id = $1 AND empresa_id = $2`,
      [serieId, empresaId],
    );
    if (!fila) throw new NotFoundError('Serie', serieId);
    return fila;
  }

  private async verificarItem(empresaId: string, itemId: string): Promise<ItemSerializado> {
    this.validarUuid(itemId, 'item');
    const item = await this.pg.queryOne<ItemSerializado>(
      `SELECT id, tipo, track_inventory, track_serial, codigo, nombre, garantia_meses
         FROM catalog.items
        WHERE id = $1 AND empresa_id = $2 AND eliminado_en IS NULL`,
      [itemId, empresaId],
    );
    if (!item) throw new NotFoundError('Item', itemId);
    return item;
  }

  private async validarUbicacion(
    empresaId: string,
    ubicacionId: string | null | undefined,
  ): Promise<string | null> {
    if (ubicacionId === null || ubicacionId === undefined || ubicacionId === '') return null;
    this.validarUuid(ubicacionId, 'ubicacion');
    const ubicacion = await this.pg.queryOne<{ id: string }>(
      `SELECT id FROM catalog.ubicaciones WHERE id = $1 AND empresa_id = $2 AND activo`,
      [ubicacionId, empresaId],
    );
    if (!ubicacion) throw new NotFoundError('Ubicacion', ubicacionId);
    return ubicacion.id;
  }

  private validarSerial(valor: unknown): string {
    const serial = String(valor ?? '').trim();
    if (!SERIAL.test(serial)) {
      throw new DomainError(
        'SERIAL_INVALIDO',
        'El serial debe tener entre 1 y 120 caracteres alfanumericos, punto, guion o guion bajo.',
      );
    }
    return serial;
  }

  private validarImei(valor: unknown): string {
    const imei = String(valor ?? '').trim();
    if (!IMEI.test(imei)) {
      throw new DomainError('IMEI_INVALIDO', 'El IMEI debe contener solo digitos (maximo 20).');
    }
    return imei;
  }

  private validarEstadoManual(valor: unknown): EstadoSerie {
    const estado = String(valor ?? '').trim() as EstadoSerie;
    if (!ESTADOS_SERIE.includes(estado) || !ESTADOS_MANUALES.includes(estado)) {
      throw new DomainError(
        'ESTADO_SERIE_INVALIDO',
        'Solo se puede fijar manualmente el estado disponible o baja.',
      );
    }
    return estado;
  }

  private validarUuid(valor: unknown, campo: string): string {
    const id = String(valor ?? '');
    if (!UUID.test(id)) throw new DomainError('ID_INVALIDO', `El ${campo} debe ser un UUID.`);
    return id;
  }

  private esConflicto(error: unknown): boolean {
    const codigo = (error as { code?: unknown })?.code;
    return codigo === '23505' || codigo === '23P01';
  }
}
