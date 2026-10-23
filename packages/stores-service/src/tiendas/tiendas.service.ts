import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  PgService,
  DomainError,
  ConflictError,
  ForbiddenError,
  NotFoundError,
  esUuid,
  esViolacionUnicidad,
  slugConSufijo,
  slugDesdeNombre,
  validarSlugSolicitado,
} from '@core/shared';

export interface Tienda {
  id: string;
  empresa_id: string;
  propietario_usuario_id: string;
  participacion_porcentaje: number | null;
  vendedor_id: string | null;
  nombre: string;
  slug: string;
  descripcion: string;
  creado_en: string;
}

export interface TiendaResuelta {
  tienda: Tienda;
  /** Ruta a la que redirigir cuando se consultó un slug anterior. */
  redirigirA?: string;
}

interface TiendaConEstadoSlug extends Tienda {
  es_actual: boolean;
}

const CAMPOS_TIENDA =
  'id, empresa_id, propietario_usuario_id, participacion_porcentaje, vendedor_id, nombre, descripcion, slug, creado_en';
const MAX_INTENTOS_SLUG = 50;

/**
 * Tiendas: espacio virtual de cada vendedor (Tabla 11). Una tienda por vendedor.
 */
@Injectable()
export class TiendasService {
  constructor(private readonly pg: PgService) {}

  async deVendedor(vendedorId: string): Promise<Tienda | null> {
    return this.pg.queryOne<Tienda>(
      `SELECT ${CAMPOS_TIENDA}
       FROM stores.tiendas WHERE vendedor_id = $1`,
      [vendedorId],
    );
  }

  async deEmpresa(empresaId: string): Promise<Tienda[]> {
    return this.pg.query<Tienda>(
      `SELECT ${CAMPOS_TIENDA}
       FROM stores.tiendas WHERE empresa_id = $1
       ORDER BY creado_en DESC LIMIT 200`,
      [empresaId],
    );
  }

  /** Resuelve por UUID o slug; conserva el slug anterior como redirección. */
  async resolver(identificador: string): Promise<TiendaResuelta | null> {
    const fila = await this.buscarPorIdentificador(identificador);
    if (!fila) return null;
    const { es_actual, ...tienda } = fila;
    return {
      tienda,
      redirigirA: es_actual ? undefined : `/api/v1/tiendas/${tienda.slug}`,
    };
  }

  async encontrarPorId(id: string): Promise<Tienda | null> {
    return (await this.resolver(id))?.tienda ?? null;
  }

  async crear(
    empresaId: string,
    propietarioId: string,
    participacion: number | null,
    nombre: string,
    descripcion?: string,
    slugSolicitado?: string,
    vendedorId: string | null = propietarioId,
  ): Promise<Tienda> {
    const nombreFinal = nombre.trim();
    if (nombreFinal.length < 2 || nombreFinal.length > 100) {
      throw new DomainError('NOMBRE_INVALIDO', 'El nombre de la tienda debe tener entre 2 y 100 caracteres.');
    }
    const descripcionFinal = (descripcion ?? '').trim();
    const slugExplicito =
      slugSolicitado === undefined ? undefined : validarSlugSolicitado(slugSolicitado);
    const maxIntentos = slugExplicito ? 1 : 3;

    for (let intento = 1; intento <= maxIntentos; intento += 1) {
      const slug = await this.elegirSlug(nombreFinal, slugExplicito);
      try {
        const fila = await this.pg.queryOne<Tienda>(
          `INSERT INTO stores.tiendas
             (id, empresa_id, propietario_usuario_id, vendedor_id, participacion_porcentaje, nombre, descripcion, slug)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING ${CAMPOS_TIENDA}`,
          [randomUUID(), empresaId, propietarioId, vendedorId, participacion, nombreFinal, descripcionFinal, slug],
        );
        if (!fila) throw new DomainError('TIENDA_FALLIDA', 'No se pudo crear la tienda.');
        return fila;
      } catch (error) {
        if (!esViolacionUnicidad(error)) throw error;
        if (slugExplicito || intento === maxIntentos) {
          throw new ConflictError(`El slug "${slugExplicito ?? slug}" ya está en uso.`);
        }
      }
    }

    throw new ConflictError('No se pudo asignar un slug disponible.');
  }

  /** Cambia el slug actual; el anterior queda en el historial y redirige 301. */
  async actualizarSlug(
    actorId: string,
    identificador: string,
    slugSolicitado: string,
  ): Promise<Tienda> {
    const resuelta = await this.resolver(identificador);
    if (!resuelta) throw new NotFoundError('Tienda', identificador);
    const tienda = resuelta.tienda;
    if (tienda.propietario_usuario_id !== actorId) {
      throw new ForbiddenError('Solo el propietario de la tienda puede cambiar su slug.');
    }

    const slug = validarSlugSolicitado(slugSolicitado);
    if (slug === tienda.slug) return tienda;
    if (!(await this.slugLibre(slug))) {
      throw new ConflictError(`El slug "${slug}" ya está en uso.`);
    }

    try {
      const fila = await this.pg.queryOne<Tienda>(
        `UPDATE stores.tiendas SET slug=$1 WHERE id=$2 RETURNING ${CAMPOS_TIENDA}`,
        [slug, tienda.id],
      );
      if (!fila) throw new NotFoundError('Tienda', tienda.id);
      return fila;
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError(`El slug "${slug}" ya está en uso.`);
      }
      throw error;
    }
  }

  private async buscarPorIdentificador(
    identificador: string,
  ): Promise<TiendaConEstadoSlug | null> {
    if (esUuid(identificador)) {
      return this.pg.queryOne<TiendaConEstadoSlug>(
        `SELECT t.*, true AS es_actual FROM stores.tiendas t WHERE t.id = $1`,
        [identificador],
      );
    }
    return this.pg.queryOne<TiendaConEstadoSlug>(
      `SELECT t.*, (s.reemplazado_en IS NULL) AS es_actual
       FROM stores.tienda_slugs s
       JOIN stores.tiendas t ON t.id = s.tienda_id
       WHERE s.slug = $1`,
      [identificador],
    );
  }

  private async elegirSlug(nombre: string, explicito?: string): Promise<string> {
    if (explicito) {
      if (!(await this.slugLibre(explicito))) {
        throw new ConflictError(`El slug "${explicito}" ya está en uso.`);
      }
      return explicito;
    }

    const base = slugDesdeNombre(nombre, 'tienda');
    for (let intento = 1; intento <= MAX_INTENTOS_SLUG; intento += 1) {
      const candidato = intento === 1 ? base : slugConSufijo(base, intento);
      if (await this.slugLibre(candidato)) return candidato;
    }
    return this.slugAleatorio(base);
  }

  private async slugLibre(slug: string): Promise<boolean> {
    const fila = await this.pg.queryOne(
      `SELECT 1 FROM stores.tienda_slugs WHERE slug=$1`,
      [slug],
    );
    return !fila;
  }

  private slugAleatorio(base: string): string {
    const sufijo = `-${Date.now().toString(36)}`;
    const maximo = Math.max(2, 80 - sufijo.length);
    const recorte = base.slice(0, maximo).replace(/-+$/g, '');
    return `${recorte || 'tienda'}${sufijo}`;
  }
}
