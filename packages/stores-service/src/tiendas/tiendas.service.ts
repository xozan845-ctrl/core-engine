import { Injectable } from '@nestjs/common';
import { randomUUID } from 'crypto';
import {
  PgService,
  DomainError,
  ConflictError,
  ForbiddenError,
  NotFoundError,
  OrigenPersonalizacion,
  esUuid,
  esViolacionUnicidad,
  slugConSufijo,
  slugDesdeNombre,
  validarSlugSolicitado,
} from '@core/shared';
import {
  BackgroundInput,
  BackgroundMediaType,
  BackgroundMovilCampos,
  BackgroundMovilInput,
  BackgroundPosition,
  ValidadorMediaBackground,
  backgroundDe,
  backgroundMovilDe,
  conBackgroundMovilUrls,
  conBackgroundUrls,
  normalizarBackground,
  normalizarBackgroundMovil,
  validarBackground,
  validarBackgroundMovil,
} from './media-background';
import {
  OverridesPersonalizacion,
  PersonalizacionCampos,
  PersonalizacionInput,
  Plantilla,
  conSeoImagenUrl,
  normalizarOverrides,
  overridesDe,
  validarOverrides,
} from './personalizacion-tienda';
import {
  ContenidoCampos,
  ContenidoInput,
  contenidoDe,
  normalizarContenido,
  validarContenido,
} from './contenido-tienda';
import { TemaHerenciaService, resolverPersonalizacion } from './herencia-tema';

export interface Tienda extends PersonalizacionCampos, ContenidoCampos, BackgroundMovilCampos {
  id: string;
  empresa_id: string;
  propietario_usuario_id: string;
  participacion_porcentaje: number | null;
  vendedor_id: string | null;
  nombre: string;
  slug: string;
  descripcion: string;
  logo_objeto_id: string | null;
  logo_url?: string | null;
  background_objeto_id: string | null;
  background_media_type: BackgroundMediaType | null;
  background_poster_objeto_id: string | null;
  background_alt: string;
  background_overlay: number;
  background_position: BackgroundPosition;
  background_activo: boolean;
  background_url?: string | null;
  background_poster_url?: string | null;
  seo_imagen_url?: string | null;
  origen_personalizacion: {
    plantilla: OrigenPersonalizacion;
    tema: OrigenPersonalizacion;
  };
  creado_en: string;
  actualizado_en: string;
}

export interface TiendaResuelta {
  tienda: Tienda;
  /** Overrides crudos para una actualización parcial (ADR-42). */
  overrides: OverridesPersonalizacion;
  /** Ruta a la que redirigir cuando se consultó un slug anterior. */
  redirigirA?: string;
}

interface TiendaFila
  extends Omit<
    Tienda,
    | 'plantilla'
    | 'tema'
    | 'origen_personalizacion'
    | 'seo_imagen_url'
    | 'logo_url'
    | 'background_url'
    | 'background_poster_url'
    | 'background_movil_url'
    | 'background_movil_poster_url'
  > {
  plantilla_override: Plantilla | null;
  tema_override: unknown;
}

interface TiendaConEstadoSlug extends TiendaFila {
  es_actual: boolean;
}

const CAMPOS_TIENDA =
  'id, empresa_id, propietario_usuario_id, participacion_porcentaje, vendedor_id, nombre, descripcion, slug, logo_objeto_id, background_objeto_id, background_media_type, background_poster_objeto_id, background_alt, background_overlay, background_position, background_activo, background_movil_objeto_id, background_movil_media_type, background_movil_poster_objeto_id, background_movil_alt, background_movil_activo, plantilla_override, tema_override, hero_titulo, hero_subtitulo, hero_cta_texto, hero_cta_url, seo_titulo, seo_descripcion, seo_imagen_objeto_id, personalizacion_actualizada_en, secciones, sobre_nosotros, politicas, redes, anuncio_texto, anuncio_enlace, anuncio_activo, contenido_actualizado_en, creado_en, actualizado_en';
const MAX_INTENTOS_SLUG = 50;

/**
 * Tiendas: espacio virtual de cada vendedor (Tabla 11). Una tienda por vendedor.
 */
@Injectable()
export class TiendasService {
  private readonly storageUrl = process.env.STORAGE_SERVICE_URL ?? 'http://storage-service:3010';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly validadorMedia = new ValidadorMediaBackground(
    this.storageUrl,
    this.claveInterna,
  );

  constructor(
    private readonly pg: PgService,
    private readonly herencia: TemaHerenciaService = new TemaHerenciaService(),
  ) {}

  async deVendedor(vendedorId: string): Promise<Tienda | null> {
    const tienda = await this.pg.queryOne<TiendaFila>(
      `SELECT ${CAMPOS_TIENDA}
       FROM stores.tiendas
       WHERE vendedor_id = $1 OR EXISTS (
         SELECT 1 FROM identity.asignaciones_personal ap
         WHERE ap.usuario_id = $1 AND ap.tienda_id = stores.tiendas.id AND ap.estado = 'activa'
       )
       ORDER BY CASE WHEN vendedor_id = $1 THEN 0 ELSE 1 END, creado_en
       LIMIT 1`,
      [vendedorId],
    );
    return tienda ? this.conUrls(tienda) : null;
  }

  async deEmpresa(empresaId: string): Promise<Tienda[]> {
    const tiendas = await this.pg.query<TiendaFila>(
      `SELECT ${CAMPOS_TIENDA}
       FROM stores.tiendas WHERE empresa_id = $1
       ORDER BY creado_en DESC LIMIT 200`,
      [empresaId],
    );
    return Promise.all(tiendas.map((tienda) => this.conUrls(tienda)));
  }

  /** Resuelve por UUID o slug; conserva el slug anterior como redirección. */
  async resolver(identificador: string): Promise<TiendaResuelta | null> {
    const fila = await this.buscarPorIdentificador(identificador);
    if (!fila) return null;
    const { es_actual, ...tienda } = fila;
    return {
      tienda: await this.conUrls(tienda),
      overrides: overridesDe(tienda),
      redirigirA: es_actual ? undefined : `/api/v2/tiendas/${tienda.slug}`,
    };
  }

  async encontrarPorId(id: string): Promise<Tienda | null> {
    return (await this.resolver(id))?.tienda ?? null;
  }

  /** Lote interno por ids (identity no debe leer `stores.tiendas`). */
  async porIds(ids: string[]): Promise<Tienda[]> {
    if (ids.length === 0) return [];
    const filas = await this.pg.query<TiendaFila>(
      `SELECT ${CAMPOS_TIENDA} FROM stores.tiendas WHERE id = ANY($1::uuid[])`,
      [ids],
    );
    return Promise.all(filas.map((fila) => this.conUrls(fila)));
  }

  /** Asignación atómica de vendedor (solo si la tienda sigue libre). */
  async asignarVendedor(
    tiendaId: string,
    empresaId: string,
    usuarioId: string,
    participacion: number,
  ): Promise<void> {
    if (!Number.isInteger(participacion) || participacion < 0 || participacion > 100) {
      throw new DomainError('PARTICIPACION_INVALIDA', 'La participación debe estar entre 0 y 100.');
    }
    const fila = await this.pg.queryOne(
      `UPDATE stores.tiendas
          SET vendedor_id = $1, propietario_usuario_id = $1,
              participacion_porcentaje = $2, actualizado_en = NOW()
        WHERE id = $3 AND empresa_id = $4 AND vendedor_id IS NULL
      RETURNING id`,
      [usuarioId, participacion, tiendaId, empresaId],
    );
    if (!fila) {
      throw new ConflictError(
        'La tienda ya tiene un vendedor asignado.',
        { tienda_id: tiendaId },
        'TIENDA_YA_ASIGNADA',
      );
    }
  }

  /** Compensación: libera la tienda si la transacción del llamador falla. */
  async desasignarVendedor(tiendaId: string, usuarioId: string): Promise<void> {
    await this.pg.query(
      `UPDATE stores.tiendas
          SET vendedor_id = NULL, participacion_porcentaje = NULL, actualizado_en = NOW()
        WHERE id = $1 AND vendedor_id = $2`,
      [tiendaId, usuarioId],
    );
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
        const fila = await this.pg.queryOne<TiendaFila>(
          `INSERT INTO stores.tiendas
             (id, empresa_id, propietario_usuario_id, vendedor_id, participacion_porcentaje, nombre, descripcion, slug)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING ${CAMPOS_TIENDA}`,
          [randomUUID(), empresaId, propietarioId, vendedorId, participacion, nombreFinal, descripcionFinal, slug],
        );
        if (!fila) throw new DomainError('TIENDA_FALLIDA', 'No se pudo crear la tienda.');
        return await this.conUrls(fila);
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
      const fila = await this.pg.queryOne<TiendaFila>(
        `UPDATE stores.tiendas SET slug=$1 WHERE id=$2 RETURNING ${CAMPOS_TIENDA}`,
        [slug, tienda.id],
      );
      if (!fila) throw new NotFoundError('Tienda', tienda.id);
      return await this.conUrls(fila);
    } catch (error) {
      if (esViolacionUnicidad(error)) {
        throw new ConflictError(`El slug "${slug}" ya está en uso.`);
      }
      throw error;
    }
  }

  /**
   * Asocia o quita el logo de la tienda. El vendedor dueño, el empresario de la
   * empresa y admin/super_admin pueden hacerlo; el objeto debe ser una imagen
   * pública del tenant de la empresa.
   */
  async actualizarLogo(
    actor: { user_id: string; rol: string; tenant_id?: string },
    identificador: string,
    logoObjetoId: string | null,
  ): Promise<Tienda> {
    const resuelta = await this.resolver(identificador);
    if (!resuelta) throw new NotFoundError('Tienda', identificador);
    const tienda = resuelta.tienda;
    this.verificarAcceso(actor, tienda, 'Solo el dueño de la tienda puede cambiar su logo.');
    if (logoObjetoId !== null) {
      if (!esUuid(logoObjetoId)) {
        throw new DomainError('LOGO_INVALIDO', 'El logo debe ser un objeto de storage válido.');
      }
      await this.validarLogo(tienda.empresa_id, logoObjetoId);
    }
    const fila = await this.pg.queryOne<TiendaFila>(
      `UPDATE stores.tiendas SET logo_objeto_id=$1, actualizado_en=NOW()
        WHERE id=$2 RETURNING ${CAMPOS_TIENDA}`,
      [logoObjetoId, tienda.id],
    );
    if (!fila) throw new NotFoundError('Tienda', tienda.id);
    return await this.conUrls(fila);
  }

  /**
   * Asocia, cambia o limpia el fondo de la tienda (ADR-38). Misma autorizacion
   * que el logo; el objeto se valida contra el tenant de la empresa dueña.
   */
  async actualizarBackground(
    actor: { user_id: string; rol: string; tenant_id?: string },
    identificador: string,
    datos: BackgroundInput & BackgroundMovilInput,
  ): Promise<Tienda> {
    const resuelta = await this.resolver(identificador);
    if (!resuelta) throw new NotFoundError('Tienda', identificador);
    const tienda = resuelta.tienda;
    this.verificarAcceso(actor, tienda, 'Solo el dueño de la tienda puede cambiar su fondo.');

    const meta = normalizarBackground(datos, backgroundDe(tienda));
    validarBackground(datos, meta);
    const metaMovil = normalizarBackgroundMovil(datos, backgroundMovilDe(tienda));
    validarBackgroundMovil(datos, metaMovil);

    if (
      meta.background_objeto_id &&
      meta.background_objeto_id !== tienda.background_objeto_id
    ) {
      meta.background_media_type = await this.validadorMedia.validar(
        tienda.empresa_id,
        meta.background_objeto_id,
      );
    }
    if (
      meta.background_poster_objeto_id &&
      meta.background_poster_objeto_id !== tienda.background_poster_objeto_id
    ) {
      await this.validadorMedia.validar(
        tienda.empresa_id,
        meta.background_poster_objeto_id,
        'poster',
      );
    }
    if (
      metaMovil.background_movil_objeto_id &&
      metaMovil.background_movil_objeto_id !== tienda.background_movil_objeto_id
    ) {
      metaMovil.background_movil_media_type = await this.validadorMedia.validar(
        tienda.empresa_id,
        metaMovil.background_movil_objeto_id,
      );
    }
    if (
      metaMovil.background_movil_poster_objeto_id &&
      metaMovil.background_movil_poster_objeto_id !==
        tienda.background_movil_poster_objeto_id
    ) {
      await this.validadorMedia.validar(
        tienda.empresa_id,
        metaMovil.background_movil_poster_objeto_id,
        'poster',
      );
    }

    const fila = await this.pg.queryOne<TiendaFila>(
      `UPDATE stores.tiendas SET
         background_objeto_id=$1, background_media_type=$2,
         background_poster_objeto_id=$3, background_alt=$4,
         background_overlay=$5, background_position=$6, background_activo=$7,
         background_movil_objeto_id=$8, background_movil_media_type=$9,
         background_movil_poster_objeto_id=$10, background_movil_alt=$11,
         background_movil_activo=$12, actualizado_en=NOW()
       WHERE id=$13 RETURNING ${CAMPOS_TIENDA}`,
      [
        meta.background_objeto_id,
        meta.background_media_type,
        meta.background_poster_objeto_id,
        meta.background_alt,
        meta.background_overlay,
        meta.background_position,
        meta.background_activo,
        metaMovil.background_movil_objeto_id,
        metaMovil.background_movil_media_type,
        metaMovil.background_movil_poster_objeto_id,
        metaMovil.background_movil_alt,
        metaMovil.background_movil_activo,
        tienda.id,
      ],
    );
    if (!fila) throw new NotFoundError('Tienda', tienda.id);
    return await this.conUrls(fila);
  }

  /**
   * Actualiza la identidad visual de la tienda (ADR-39): plantilla, tema,
   * hero y SEO. Misma autorización que el logo/fondo; la imagen SEO se valida
   * contra storage con el tenant de la empresa dueña.
   */
  async actualizarPersonalizacion(
    actor: { user_id: string; rol: string; tenant_id?: string },
    identificador: string,
    datos: PersonalizacionInput,
  ): Promise<Tienda> {
    const resuelta = await this.resolver(identificador);
    if (!resuelta) throw new NotFoundError('Tienda', identificador);
    const tienda = resuelta.tienda;
    this.verificarAcceso(actor, tienda, 'Solo el dueño de la tienda puede personalizarla.');

    const meta = normalizarOverrides(datos, resuelta.overrides);
    validarOverrides(meta);

    if (
      meta.seo_imagen_objeto_id &&
      meta.seo_imagen_objeto_id !== resuelta.overrides.seo_imagen_objeto_id
    ) {
      await this.validadorMedia.validar(
        tienda.empresa_id,
        meta.seo_imagen_objeto_id,
        'imagen',
      );
    }

    const fila = await this.pg.queryOne<TiendaFila>(
      `UPDATE stores.tiendas SET
         plantilla_override=$1, tema_override=$2::jsonb, hero_titulo=$3,
         hero_subtitulo=$4, hero_cta_texto=$5, hero_cta_url=$6, seo_titulo=$7,
         seo_descripcion=$8, seo_imagen_objeto_id=$9,
         personalizacion_actualizada_en=NOW(), actualizado_en=NOW()
       WHERE id=$10 RETURNING ${CAMPOS_TIENDA}`,
      [
        meta.plantilla,
        JSON.stringify(meta.tema),
        meta.hero_titulo,
        meta.hero_subtitulo,
        meta.hero_cta_texto,
        meta.hero_cta_url,
        meta.seo_titulo,
        meta.seo_descripcion,
        meta.seo_imagen_objeto_id,
        tienda.id,
      ],
    );
    if (!fila) throw new NotFoundError('Tienda', tienda.id);
    return await this.conUrls(fila);
  }

  /**
   * Actualiza el contenido del storefront (ADR-40): secciones, sobre nosotros,
   * políticas, redes y anuncio. Misma autorización que el logo/fondo.
   */
  async actualizarContenido(
    actor: { user_id: string; rol: string; tenant_id?: string },
    identificador: string,
    datos: ContenidoInput,
  ): Promise<Tienda> {
    const resuelta = await this.resolver(identificador);
    if (!resuelta) throw new NotFoundError('Tienda', identificador);
    const tienda = resuelta.tienda;
    this.verificarAcceso(
      actor,
      tienda,
      'Solo el dueño de la tienda puede cambiar su contenido.',
    );

    const meta = normalizarContenido(datos, contenidoDe(tienda));
    validarContenido(meta);

    const fila = await this.pg.queryOne<TiendaFila>(
      `UPDATE stores.tiendas SET
         secciones=$1::jsonb, sobre_nosotros=$2, politicas=$3::jsonb, redes=$4::jsonb,
         anuncio_texto=$5, anuncio_enlace=$6, anuncio_activo=$7,
         contenido_actualizado_en=NOW(), actualizado_en=NOW()
       WHERE id=$8 RETURNING ${CAMPOS_TIENDA}`,
      [
        JSON.stringify(meta.secciones),
        meta.sobre_nosotros,
        JSON.stringify(meta.politicas),
        JSON.stringify(meta.redes),
        meta.anuncio_texto,
        meta.anuncio_enlace,
        meta.anuncio_activo,
        tienda.id,
      ],
    );
    if (!fila) throw new NotFoundError('Tienda', tienda.id);
    return await this.conUrls(fila);
  }

  private verificarAcceso(
    actor: { user_id: string; rol: string; tenant_id?: string },
    tienda: Tienda,
    mensaje: string,
  ): void {
    const esGlobal = actor.rol === 'super_admin' || actor.rol === 'admin';
    const esEmpresario =
      actor.rol === 'empresario' && !!actor.tenant_id && actor.tenant_id === tienda.empresa_id;
    const esVendedor =
      actor.rol === 'vendedor' &&
      (tienda.propietario_usuario_id === actor.user_id || tienda.vendedor_id === actor.user_id);
    if (!esGlobal && !esEmpresario && !esVendedor) throw new ForbiddenError(mensaje);
  }

  private async validarLogo(empresaId: string, objetoId: string): Promise<void> {
    let respuesta: Response;
    try {
      respuesta = await fetch(
        `${this.storageUrl}/internal/storage/objetos/${encodeURIComponent(objetoId)}` +
          `?tenant_id=${encodeURIComponent(empresaId)}`,
        {
          headers: { 'x-internal-key': this.claveInterna },
          // R-RB-4: fallar rapido en vez de esperar el timeout del proxy.
          signal: AbortSignal.timeout(4000),
        },
      );
    } catch {
      throw new DomainError('LOGO_NO_VERIFICABLE', 'No se pudo verificar el logo.');
    }
    if (respuesta.status === 404) throw new NotFoundError('Logo', objetoId);
    if (!respuesta.ok) {
      throw new DomainError('LOGO_NO_VERIFICABLE', 'No se pudo verificar el logo.');
    }
    const objeto = (await respuesta.json()) as {
      tenant_id?: string;
      estado?: string;
      publico?: boolean;
      content_type?: string;
    };
    if (
      objeto.tenant_id !== empresaId ||
      objeto.estado !== 'activo' ||
      objeto.publico !== true ||
      !String(objeto.content_type ?? '').startsWith('image/')
    ) {
      throw new DomainError(
        'LOGO_INVALIDO',
        'El logo debe ser una imagen pública y activa de la empresa.',
      );
    }
  }

  private async conUrls(fila: TiendaFila): Promise<Tienda> {
    const { plantilla_override: _plantilla, tema_override: _tema, ...resto } = fila;
    const overrides = overridesDe(fila);
    const [empresa, plataforma] = await Promise.all([
      this.herencia.obtenerEmpresa(fila.empresa_id),
      this.herencia.obtenerPlataforma(),
    ]);
    const resuelta = resolverPersonalizacion(overrides, empresa, plataforma);
    const personalizacion: PersonalizacionCampos = {
      ...overrides,
      plantilla: resuelta.plantilla,
      tema: resuelta.tema,
    };
    const logo = fila.logo_objeto_id
      ? { logo_url: `/api/v2/storage/public/objetos/${fila.logo_objeto_id}` }
      : {};
    return {
      ...resto,
      ...logo,
      ...conBackgroundUrls(backgroundDe(fila)),
      ...conBackgroundMovilUrls(backgroundMovilDe(fila)),
      ...personalizacion,
      ...conSeoImagenUrl(personalizacion),
      ...contenidoDe(fila),
      origen_personalizacion: resuelta.origen,
    };
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
