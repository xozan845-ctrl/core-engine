//! API keys de clientes máquina (ADR-0005, R-GW-8).
//!
//! La clave se muestra una vez y se guarda hasheada. Se intercambia por un JWT
//! corto; nunca viaja a los microservicios ni se acepta en un navegador.

use crate::auth::{crear_access_token, UsuarioContexto, UsuarioToken};
use crate::config::Config;
use crate::db::{iso_ms, uuid_texto, Db};
use crate::errors::ApiError;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;

pub const ROLES_API_KEY: [&str; 6] = [
    "empresario",
    "vendedor",
    "logistica",
    "coordinador",
    "supervisor",
    "operativo",
];

const COLUMNAS: &str =
    "id, empresa_id, nombre, prefijo, rol, estado, expira_en, creado_en, ultimo_uso_en";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApiKey {
    pub id: String,
    pub empresa_id: String,
    pub nombre: String,
    pub prefijo: String,
    pub rol: String,
    pub estado: String,
    pub expira_en: Option<String>,
    pub creado_en: String,
    pub ultimo_uso_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApiKeyCreada {
    #[serde(flatten)]
    pub api_key: ApiKey,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TokenApiKey {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrearApiKey {
    pub nombre: String,
    pub rol: String,
    pub empresa_id: Option<String>,
    pub expira_en_dias: Option<i64>,
}

pub struct ApiKeys {
    db: Arc<Db>,
    secreto: String,
    access_ttl: u64,
}

impl ApiKeys {
    pub fn nueva(db: Arc<Db>, config: &Config) -> Self {
        Self {
            db,
            secreto: config.jwt_secret.clone(),
            access_ttl: config.access_ttl_segundos(),
        }
    }

    pub async fn crear(
        &self,
        actor: &UsuarioContexto,
        datos: CrearApiKey,
    ) -> Result<ApiKeyCreada, ApiError> {
        if !ROLES_API_KEY.contains(&datos.rol.as_str()) {
            return Err(ApiError::dominio(
                "ROL_NO_PERMITIDO",
                format!("Una API key no puede tener el rol \"{}\".", datos.rol),
            ));
        }
        let empresa_id = self.empresa_de(actor, datos.empresa_id.as_deref())?;
        let prefijo = prefijo_aleatorio();
        let secreto = secreto_aleatorio();
        let clave = format!("cek_{prefijo}_{secreto}");
        let expira_en = datos
            .expira_en_dias
            .map(|dias| Utc::now() + Duration::days(dias));
        let hash = hash_de(&clave);

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO identity.api_keys
               (id, empresa_id, nombre, prefijo, hash, rol, expira_en, creado_por)
             VALUES (gen_random_uuid(), $1::uuid, $2, $3, $4, $5, $6, $7::uuid)
             RETURNING {COLUMNAS}"
        ))
        .bind(&empresa_id)
        .bind(datos.nombre.trim())
        .bind(&prefijo)
        .bind(&hash)
        .bind(&datos.rol)
        .bind(expira_en)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::dominio("PERSISTENCIA_FALLIDA", "No se pudo crear la API key."))?;
        tx.commit().await.map_err(interno)?;
        Ok(ApiKeyCreada {
            api_key: fila_a_api_key(&fila)?,
            key: clave,
        })
    }

    pub async fn listar(
        &self,
        actor: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<Vec<ApiKey>, ApiError> {
        let empresa = self.empresa_de(actor, empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM identity.api_keys
              WHERE empresa_id = $1::uuid
              ORDER BY creado_en DESC"
        ))
        .bind(&empresa)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas.iter().map(fila_a_api_key).collect()
    }

    pub async fn revocar(
        &self,
        actor: &UsuarioContexto,
        id: &str,
    ) -> Result<(String, String), ApiError> {
        let empresa = self.empresa_de(actor, None)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.api_keys SET estado = 'revocada'
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'activa'
              RETURNING id, estado",
        )
        .bind(id)
        .bind(&empresa)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("API key activa", id))?;
        tx.commit().await.map_err(interno)?;
        Ok((
            uuid_texto(&fila, "id").map_err(interno)?,
            fila.try_get("estado").map_err(interno)?,
        ))
    }

    /// Intercambia una key válida por un JWT de acceso corto (sin refresh).
    pub async fn token(&self, clave: Option<&str>) -> Result<TokenApiKey, ApiError> {
        let Some(clave) = clave
            .map(str::trim)
            .filter(|valor| valor.starts_with("cek_"))
        else {
            return Err(ApiError::no_autorizado("API key invalida o expirada."));
        };
        let hash = hash_de(clave);
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query("SELECT set_config('request.api_key_hash', $1, true)")
            .bind(&hash)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM identity.api_keys
              WHERE hash = $1 AND estado = 'activa' LIMIT 1"
        ))
        .bind(&hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_autorizado("API key invalida o expirada."))?;
        let api_key = fila_a_api_key(&fila)?;
        if let Some(expira) = &api_key.expira_en {
            if let Ok(expira) = chrono::DateTime::parse_from_rfc3339(expira) {
                if expira.with_timezone(&Utc) <= Utc::now() {
                    return Err(ApiError::no_autorizado("API key invalida o expirada."));
                }
            }
        }
        sqlx::query("UPDATE identity.api_keys SET ultimo_uso_en = NOW() WHERE id = $1::uuid")
            .bind(&api_key.id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let usuario = UsuarioToken {
            id: format!("api-key:{}", api_key.id),
            email: format!("api-key+{}@core-engine.local", api_key.prefijo),
            rol: api_key.rol.clone(),
            nombre: api_key.nombre.clone(),
            tenant_id: Some(api_key.empresa_id.clone()),
            personal_id: None,
            participacion_porcentaje: None,
            rol_empresa_id: None,
        };
        Ok(TokenApiKey {
            access_token: crear_access_token(&usuario, &self.secreto, self.access_ttl)?,
            token_type: "Bearer".to_string(),
            expires_in: self.access_ttl,
        })
    }

    fn empresa_de(
        &self,
        actor: &UsuarioContexto,
        empresa_id: Option<&str>,
    ) -> Result<String, ApiError> {
        if actor.es_admin_global() {
            return empresa_id
                .map(str::trim)
                .filter(|valor| !valor.is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    ApiError::dominio(
                        "EMPRESA_REQUERIDA",
                        "Indica empresa_id para gestionar API keys como administrador.",
                    )
                });
        }
        if actor.rol != "empresario" {
            return Err(ApiError::acceso_denegado(
                "Solo el empresario o un administrador gestiona API keys.",
            ));
        }
        actor
            .tenant_id
            .clone()
            .ok_or_else(|| ApiError::dominio("TENANT_REQUERIDO", "Se requiere una empresa activa."))
    }
}

fn fila_a_api_key(fila: &sqlx::postgres::PgRow) -> Result<ApiKey, ApiError> {
    let expira_en: Option<DateTime<Utc>> = fila.try_get("expira_en").map_err(interno)?;
    let creado_en: DateTime<Utc> = fila.try_get("creado_en").map_err(interno)?;
    let ultimo_uso_en: Option<DateTime<Utc>> = fila.try_get("ultimo_uso_en").map_err(interno)?;
    Ok(ApiKey {
        id: uuid_texto(fila, "id").map_err(interno)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(interno)?,
        nombre: fila.try_get("nombre").map_err(interno)?,
        prefijo: fila.try_get("prefijo").map_err(interno)?,
        rol: fila.try_get("rol").map_err(interno)?,
        estado: fila.try_get("estado").map_err(interno)?,
        expira_en: expira_en.map(iso_ms),
        creado_en: iso_ms(creado_en),
        ultimo_uso_en: ultimo_uso_en.map(iso_ms),
    })
}

fn prefijo_aleatorio() -> String {
    let mut bytes = [0u8; 4];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn secreto_aleatorio() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_de(clave: &str) -> String {
    format!("{:x}", Sha256::digest(clave.as_bytes()))
}

pub fn rol_api_key_valido(rol: &str) -> bool {
    ROLES_API_KEY.contains(&rol)
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

/// La clave completa solo se muestra al crearla; este helper existe para los
/// tests de contrato del formato.
pub fn formato_clave(prefijo: &str, secreto: &str) -> String {
    format!("cek_{prefijo}_{secreto}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_generar_el_formato_cek_y_un_hash_sha256() {
        let clave = formato_clave("deadbeef", "secreto");
        assert_eq!(clave, "cek_deadbeef_secreto");
        assert_eq!(hash_de(&clave).len(), 64);
        assert_ne!(hash_de("a"), hash_de("b"));
        assert!(rol_api_key_valido("empresario"));
        assert!(!rol_api_key_valido("admin"));
    }

    #[test]
    fn debe_generar_prefijo_hex_de_8_caracteres() {
        let prefijo = prefijo_aleatorio();
        assert_eq!(prefijo.len(), 8);
        assert!(prefijo.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(prefijo, prefijo_aleatorio());
    }
}
