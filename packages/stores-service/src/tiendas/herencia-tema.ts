import { TemaHerencia, plantillaDe, temaParcialDe } from '@core/shared';

export {
  resolverPersonalizacion,
  type PersonalizacionResuelta,
  type TemaHerencia,
} from '@core/shared';

interface RespuestaTema {
  plantilla_override?: unknown;
  tema_override?: unknown;
}

interface CacheTema {
  valor: TemaHerencia;
  expira: number;
}

/**
 * Obtiene los overrides de empresa y plataforma desde identity-service.
 * Cachea con TTL (R-AR-7) y degrada a "sin herencia" si identity no responde
 * (R-RB-4): la tienda conserva sus overrides y los defaults.
 */
export class TemaHerenciaService {
  private readonly identityUrl =
    process.env.IDENTITY_SERVICE_URL ?? 'http://identity-service:3001';
  private readonly claveInterna = process.env.INTERNAL_API_KEY ?? '';
  private readonly ttlMs = Number(process.env.TEMA_HERENCIA_TTL_MS ?? 60_000);
  private readonly cache = new Map<string, CacheTema>();

  async obtenerEmpresa(empresaId: string): Promise<TemaHerencia> {
    return this.obtener(
      `empresa:${empresaId}`,
      `${this.identityUrl}/internal/empresas/${encodeURIComponent(empresaId)}/tema`,
    );
  }

  async obtenerPlataforma(): Promise<TemaHerencia> {
    return this.obtener(
      'plataforma',
      `${this.identityUrl}/internal/configuracion-sitio/tema`,
    );
  }

  private async obtener(clave: string, url: string): Promise<TemaHerencia> {
    const ahora = Date.now();
    const cacheado = this.cache.get(clave);
    if (cacheado && cacheado.expira > ahora) return cacheado.valor;

    let valor: TemaHerencia = { plantilla: null, tema: {} };
    try {
      const respuesta = await fetch(url, {
        headers: { 'x-internal-key': this.claveInterna },
        signal: AbortSignal.timeout(2000),
      });
      if (respuesta.ok) {
        const datos = (await respuesta.json()) as RespuestaTema;
        valor = {
          plantilla: plantillaDe(datos.plantilla_override),
          tema: temaParcialDe(datos.tema_override),
        };
      }
    } catch {
      // R-RB-4: identity caído no bloquea el storefront.
    }

    if (this.cache.size > 500) this.cache.clear();
    this.cache.set(clave, { valor, expira: ahora + this.ttlMs });
    return valor;
  }
}
