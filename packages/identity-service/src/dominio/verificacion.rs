//! Verificación de correo (ADR-0036).
//!
//! Token de un solo uso con hash SHA-256 en BD, vigencia de 24 h y enlace al
//! frontend. Reenviar invalida los tokens anteriores. La solicitud de reenvío
//! responde igual exista o no la cuenta.

use crate::db::Db;
use crate::dominio::usuarios::Usuarios;
use crate::email::{Email, EmailMessage};
use crate::errors::ApiError;
use axum::http::StatusCode;
use base64::Engine;
use chrono::{DateTime, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;

const TOKEN_TTL_HORAS: i64 = 24;

pub struct VerificacionEmail {
    db: Arc<Db>,
    usuarios: Arc<Usuarios>,
    email: Arc<dyn Email>,
    frontend_url: String,
}

impl VerificacionEmail {
    pub fn nueva(
        db: Arc<Db>,
        usuarios: Arc<Usuarios>,
        email: Arc<dyn Email>,
        frontend_url: String,
    ) -> Self {
        Self {
            db,
            usuarios,
            email,
            frontend_url,
        }
    }

    pub async fn solicitar(&self, usuario_id: &str, correo: &str) -> Result<(), ApiError> {
        let token = token_aleatorio();
        let token_hash = hash_token(&token);
        let expira_en = Utc::now() + chrono::Duration::hours(TOKEN_TTL_HORAS);

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE identity.correo_verificacion_tokens SET usado_en = NOW()
              WHERE usuario_id = $1::uuid AND usado_en IS NULL",
        )
        .bind(usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.correo_verificacion_tokens
               (usuario_id, token_hash, expira_en)
             VALUES ($1::uuid, $2, $3)",
        )
        .bind(usuario_id)
        .bind(&token_hash)
        .bind(expira_en)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        self.enviar(correo, &token, &token_hash).await;
        Ok(())
    }

    pub async fn reenviar(&self, correo: &str) -> Result<(), ApiError> {
        let Some(usuario) = self.usuarios.encontrar_por_correo(correo).await? else {
            return Ok(());
        };
        if self.usuarios.correo_verificado(&usuario.usuario.id).await? {
            return Ok(());
        }
        self.solicitar(&usuario.usuario.id, &usuario.usuario.correo)
            .await
    }

    pub async fn confirmar(&self, token: &str) -> Result<(), ApiError> {
        let token_hash = hash_token(token.trim());
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT usuario_id, expira_en, usado_en
               FROM identity.correo_verificacion_tokens
              WHERE token_hash = $1
              FOR UPDATE",
        )
        .bind(&token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let usuario_id = validar_token(fila)?;
        sqlx::query(
            "UPDATE identity.usuarios
                SET correo_verificado_en = COALESCE(correo_verificado_en, NOW()),
                    actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(&usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.correo_verificacion_tokens SET usado_en = NOW()
              WHERE usuario_id = $1::uuid AND usado_en IS NULL",
        )
        .bind(&usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    async fn enviar(&self, correo: &str, token: &str, token_hash: &str) {
        let enlace = format!("{}/verificar-correo?token={token}", self.frontend_url);
        let mensaje = EmailMessage {
            to: correo.to_string(),
            subject: "Verifica tu correo".to_string(),
            text: format!(
                "Confirma tu correo para activar todas las funciones de tu cuenta. Abre este enlace (válido 24 horas): {enlace}"
            ),
            html: Some(format!(
                "<p>Confirma tu correo para activar todas las funciones de tu cuenta.</p><p><a href=\"{enlace}\">Verificar correo</a> (válido 24 horas).</p>"
            )),
        };
        let email = Arc::clone(&self.email);
        let envio = tokio::task::spawn_blocking(move || email.enviar(&mensaje)).await;
        if !matches!(envio, Ok(Ok(()))) {
            crate::logs::error(
                "identity.verificacion",
                crate::campos! { "msg": "email_verificacion_fallido" },
            );
            // Sin correo entregado, el token no debe quedar vivo.
            let _ = sqlx::query(
                "UPDATE identity.correo_verificacion_tokens SET usado_en = NOW()
                  WHERE token_hash = $1",
            )
            .bind(token_hash)
            .execute(self.db.pool())
            .await;
        }
    }
}

fn validar_token(fila: Option<sqlx::postgres::PgRow>) -> Result<String, ApiError> {
    let Some(fila) = fila else {
        return Err(ApiError::nueva(
            "TOKEN_INVALIDO",
            "El enlace no es válido.",
            StatusCode::GONE,
        ));
    };
    let expira_en: DateTime<Utc> = fila.try_get("expira_en").map_err(interno)?;
    let usado_en: Option<DateTime<Utc>> = fila.try_get("usado_en").map_err(interno)?;
    if usado_en.is_some() {
        return Err(ApiError::nueva(
            "TOKEN_USADO",
            "El enlace ya fue utilizado.",
            StatusCode::GONE,
        ));
    }
    if expira_en <= Utc::now() {
        return Err(ApiError::nueva(
            "TOKEN_EXPIRADO",
            "El enlace ha expirado.",
            StatusCode::GONE,
        ));
    }
    let usuario_id: uuid::Uuid = fila.try_get("usuario_id").map_err(interno)?;
    Ok(usuario_id.to_string())
}

fn token_aleatorio() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_token_es_aleatorio_y_url_safe() {
        let primero = token_aleatorio();
        let segundo = token_aleatorio();
        assert_ne!(primero, segundo);
        assert!(primero
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn el_hash_es_sha256_estable() {
        assert_eq!(hash_token("token"), hash_token("token"));
        assert_ne!(hash_token("token"), hash_token("otro"));
    }
}
