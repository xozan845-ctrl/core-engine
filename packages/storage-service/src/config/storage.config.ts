import {
  STORAGE_BUCKET_PREFIX_DEFAULT,
  STORAGE_CONTENT_TYPES_DEFAULT,
  STORAGE_CUOTA_DEFAULT_BYTES,
  STORAGE_MAX_OBJETO_BYTES_DEFAULT,
  STORAGE_URL_TTL_SEGUNDOS_DEFAULT,
} from '@core/shared';

/**
 * Configuracion del almacenamiento de objetos. Los secretos llegan por
 * variables de entorno (R-CD-7: nunca en la imagen) y los limites tienen
 * valores por defecto seguros.
 */
export interface StorageConfig {
  /** Host de MinIO (sin esquema), p. ej. `minio` dentro de la red del compose. */
  endpoint: string;
  puerto: number;
  usarSsl: boolean;
  /**
   * Endpoint que va FIRMADO en las URLs prefirmadas (el que resuelve el cliente
   * final, p. ej. el dominio publico de MinIO). Si no se define, se usa el
   * interno (válido solo cuando el cliente comparte la red del compose).
   */
  publicEndpoint: string;
  publicPuerto: number;
  publicUsarSsl: boolean;
  accessKey: string;
  secretKey: string;
  /** Region S3 para firmar (MinIO por defecto `us-east-1`). Evita la consulta
   * de region al firmar, que exigiria alcanzar el endpoint publico. */
  region: string;
  /** Prefijo de los buckets por tenant (por defecto `t-`). */
  prefijoBucket: string;
  /** Tamano maximo de un objeto (bytes). */
  maxObjetoBytes: number;
  /** Cuota por defecto de un tenant nuevo (bytes). */
  cuotaDefaultBytes: number;
  /** Validez de las URLs prefirmadas (segundos). */
  urlTtlSegundos: number;
  /** Whitelist de content-types aceptados en subidas. */
  contentTypesPermitidos: string[];
}

/** Token de inyeccion de la configuracion. */
export const STORAGE_CONFIG = 'STORAGE_CONFIG';

function enteroPositivo(valor: string | undefined, porDefecto: number): number {
  const numero = Number(valor);
  return Number.isFinite(numero) && numero > 0 ? Math.trunc(numero) : porDefecto;
}

function lista(valor: string | undefined): string[] {
  return (valor ?? '')
    .split(',')
    .map((item) => item.trim().toLowerCase())
    .filter(Boolean);
}

/**
 * Lee la configuracion del entorno. No valida secretos aqui: la validacion
 * ocurre en `onModuleInit` de la implementacion (patron del repo, para que el
 * `swagger:export` pueda construir el modulo sin credenciales ni conexiones).
 */
export function leerConfig(env: NodeJS.ProcessEnv = process.env): StorageConfig {
  const accessKey = env.MINIO_ROOT_USER ?? '';
  const secretKey = env.MINIO_ROOT_PASSWORD ?? '';
  const contentTypesPermitidos = lista(env.STORAGE_CONTENT_TYPES_PERMITIDOS);
  const endpoint = env.MINIO_ENDPOINT ?? 'minio';
  const puerto = enteroPositivo(env.MINIO_PORT, 9000);
  const usarSsl = (env.MINIO_USE_SSL ?? 'false').toLowerCase() === 'true';
  return {
    endpoint,
    puerto,
    usarSsl,
    publicEndpoint: env.MINIO_PUBLIC_ENDPOINT ?? endpoint,
    publicPuerto: enteroPositivo(env.MINIO_PUBLIC_PORT, puerto),
    publicUsarSsl:
      env.MINIO_PUBLIC_USE_SSL === undefined ? usarSsl : env.MINIO_PUBLIC_USE_SSL.toLowerCase() === 'true',
    accessKey,
    secretKey,
    region: env.MINIO_REGION ?? 'us-east-1',
    prefijoBucket: env.STORAGE_BUCKET_PREFIX ?? STORAGE_BUCKET_PREFIX_DEFAULT,
    maxObjetoBytes: enteroPositivo(env.STORAGE_MAX_OBJETO_BYTES, STORAGE_MAX_OBJETO_BYTES_DEFAULT),
    cuotaDefaultBytes: enteroPositivo(
      env.STORAGE_CUOTA_DEFAULT_BYTES ?? env.STORAGE_CUOTA_DEFAULT,
      STORAGE_CUOTA_DEFAULT_BYTES,
    ),
    urlTtlSegundos: enteroPositivo(env.STORAGE_URL_TTL_SEGUNDOS, STORAGE_URL_TTL_SEGUNDOS_DEFAULT),
    contentTypesPermitidos: contentTypesPermitidos.length
      ? contentTypesPermitidos
      : [...STORAGE_CONTENT_TYPES_DEFAULT],
  };
}
