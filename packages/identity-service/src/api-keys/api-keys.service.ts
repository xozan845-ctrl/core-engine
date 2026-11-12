import { Injectable } from '@nestjs/common';
import { createHash, randomBytes, randomUUID } from 'crypto';
import {
  crearAccessToken,
  DomainError,
  ForbiddenError,
  JWT_ACCESS_TTL_DEFAULT,
  NotFoundError,
  PgService,
  ROLES,
  Rol,
  secretoJwt,
  UnauthorizedError,
  UsuarioContexto,
} from '@core/shared';
import { ttlAMs } from '../auth/auth.service';

/** Roles con los que se puede emitir una key (nunca admin/super_admin). */
export const ROLES_API_KEY: Rol[] = [
  ROLES.EMPRESARIO,
  ROLES.VENDEDOR,
  ROLES.LOGISTICA,
  ROLES.COORDINADOR,
  ROLES.SUPERVISOR,
  ROLES.OPERATIVO,
];

export interface ApiKey {
  id: string;
  empresa_id: string;
  nombre: string;
  prefijo: string;
  rol: Rol;
  estado: 'activa' | 'revocada';
  expira_en: string | null;
  creado_en: string;
  ultimo_uso_en: string | null;
}

export interface ApiKeyCreada extends ApiKey {
  /** Se devuelve una sola vez; no se puede recuperar después. */
  key: string;
}

export interface TokenApiKey {
  access_token: string;
  token_type: 'Bearer';
  expires_in: number;
}

const COLUMNAS = `id, empresa_id, nombre, prefijo, rol, estado, expira_en, creado_en, ultimo_uso_en`;

function hashDe(clave: string): string {
  return createHash('sha256').update(clave, 'utf8').digest('hex');
}

/**
 * API keys de clientes máquina (ADR-0005, R-GW-8).
 *
 * La clave se muestra una vez y se guarda hasheada. Se intercambia por un JWT
 * corto; nunca viaja a los microservicios ni se acepta en un navegador.
 */
@Injectable()
export class ApiKeysService {
  constructor(private readonly pg: PgService) {}

  async crear(
    actor: UsuarioContexto,
    dto: { nombre: string; rol: Rol; empresa_id?: string; expira_en_dias?: number },
  ): Promise<ApiKeyCreada> {
    if (!ROLES_API_KEY.includes(dto.rol)) {
      throw new DomainError(
        'ROL_NO_PERMITIDO',
        `Una API key no puede tener el rol "${dto.rol}".`,
      );
    }
    const empresaId = this.empresaDe(actor, dto.empresa_id);
    const prefijo = randomBytes(4).toString('hex');
    const secreto = randomBytes(32).toString('base64url');
    const clave = `cek_${prefijo}_${secreto}`;
    const expira = dto.expira_en_dias
      ? new Date(Date.now() + dto.expira_en_dias * 86_400_000).toISOString()
      : null;

    const fila = await this.pg.queryOne<Omit<ApiKey, 'key'>>(
      `INSERT INTO identity.api_keys
         (id, empresa_id, nombre, prefijo, hash, rol, expira_en, creado_por)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
       RETURNING ${COLUMNAS}`,
      [
        randomUUID(),
        empresaId,
        dto.nombre.trim(),
        prefijo,
        hashDe(clave),
        dto.rol,
        expira,
        actor.user_id,
      ],
    );
    if (!fila) throw new DomainError('PERSISTENCIA_FALLIDA', 'No se pudo crear la API key.');
    return { ...fila, key: clave };
  }

  async listar(actor: UsuarioContexto, empresaId?: string): Promise<ApiKey[]> {
    const empresa = this.empresaDe(actor, empresaId);
    return this.pg.query<ApiKey>(
      `SELECT ${COLUMNAS} FROM identity.api_keys
        WHERE empresa_id = $1 ORDER BY creado_en DESC`,
      [empresa],
    );
  }

  async revocar(actor: UsuarioContexto, id: string): Promise<{ id: string; estado: string }> {
    const empresa = this.empresaDe(actor);
    const fila = await this.pg.queryOne<{ id: string; estado: string }>(
      `UPDATE identity.api_keys SET estado = 'revocada'
        WHERE id = $1 AND empresa_id = $2 AND estado = 'activa'
        RETURNING id, estado`,
      [id, empresa],
    );
    if (!fila) throw new NotFoundError('API key activa', id);
    return fila;
  }

  /** Intercambia una key válida por un JWT de acceso corto (sin refresh). */
  async token(clave: string | undefined): Promise<TokenApiKey> {
    if (!clave || !clave.startsWith('cek_')) {
      throw new UnauthorizedError('API key invalida o expirada.');
    }
    const fila = await this.pg.queryOne<ApiKey>(
      `SELECT ${COLUMNAS} FROM identity.api_keys
        WHERE hash = $1 AND estado = 'activa' LIMIT 1`,
      [hashDe(clave)],
    );
    if (!fila) throw new UnauthorizedError('API key invalida o expirada.');
    if (fila.expira_en && new Date(fila.expira_en).getTime() <= Date.now()) {
      throw new UnauthorizedError('API key invalida o expirada.');
    }

    await this.pg.query(`UPDATE identity.api_keys SET ultimo_uso_en = NOW() WHERE id = $1`, [
      fila.id,
    ]);

    const ttl = process.env.JWT_ACCESS_TTL ?? JWT_ACCESS_TTL_DEFAULT;
    const accessToken = crearAccessToken(
      {
        id: `api-key:${fila.id}`,
        email: `api-key+${fila.prefijo}@core-engine.local`,
        rol: fila.rol,
        nombre: fila.nombre,
        tenant_id: fila.empresa_id,
      },
      secretoJwt(),
      ttl,
    );
    return {
      access_token: accessToken,
      token_type: 'Bearer',
      expires_in: Math.floor(ttlAMs(ttl) / 1000),
    };
  }

  private empresaDe(actor: UsuarioContexto, empresaId?: string): string {
    if ([ROLES.SUPER_ADMIN, ROLES.ADMIN].includes(actor.rol as never)) {
      if (!empresaId) {
        throw new DomainError(
          'EMPRESA_REQUERIDA',
          'Indica empresa_id para gestionar API keys como administrador.',
        );
      }
      return empresaId;
    }
    if (actor.rol !== ROLES.EMPRESARIO) {
      throw new ForbiddenError('Solo el empresario o un administrador gestiona API keys.');
    }
    if (!actor.tenant_id) {
      throw new DomainError('TENANT_REQUERIDO', 'Se requiere una empresa activa.');
    }
    return actor.tenant_id;
  }
}
