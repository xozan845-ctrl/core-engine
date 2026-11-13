//! Restablecimiento de contraseña (ADR-0003, fase 2).
//!
//! - El token en claro solo viaja en el correo; en BD queda su hash SHA-256.
//! - Un solo uso y 30 minutos de vigencia; solicitar uno nuevo invalida los
//!   anteriores del mismo usuario.
//! - La respuesta de la solicitud nunca revela si el correo existe (OWASP A07).

use crate::auth::hash_contrasena;
use crate::db::Db;
use crate::dominio::sesiones::Sesiones;
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

const TOKEN_TTL_MINUTOS: i64 = 30;

pub struct PasswordReset {
    db: Arc<Db>,
    usuarios: Arc<Usuarios>,
    sesiones: Arc<Sesiones>,
    email: Arc<dyn Email>,
    frontend_url: String,
}

impl PasswordReset {
    pub fn nueva(
        db: Arc<Db>,
        usuarios: Arc<Usuarios>,
        sesiones: Arc<Sesiones>,
        email: Arc<dyn Email>,
        frontend_url: String,
    ) -> Self {
        Self {
            db,
            usuarios,
            sesiones,
            email,
            frontend_url,
        }
    }

    pub async fn solicitar(&self, correo: &str) -> Result<(), ApiError> {
        let Some(usuario) = self.usuarios.encontrar_por_correo(correo).await? else {
            return Ok(());
        };
        let token = token_aleatorio();
        let token_hash = hash_token(&token);
        let expira_en = Utc::now() + chrono::Duration::minutes(TOKEN_TTL_MINUTOS);

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE identity.password_reset_tokens SET usado_en = NOW()
              WHERE usuario_id = $1::uuid AND usado_en IS NULL",
        )
        .bind(&usuario.usuario.id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.password_reset_tokens (usuario_id, token_hash, expira_en)
             VALUES ($1::uuid, $2, $3)",
        )
        .bind(&usuario.usuario.id)
        .bind(&token_hash)
        .bind(expira_en)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let enlace = format!("{}/restablecer-contrasena?token={token}", self.frontend_url);
        let mensaje = EmailMessage {
            to: usuario.usuario.correo.clone(),
            subject: "Restablece tu contraseña".to_string(),
            text: format!(
                "Recibimos una solicitud para restablecer tu contraseña. Abre este enlace (válido 30 minutos): {enlace}\n\nSi no fuiste tú, ignora este correo."
            ),
            html: Some(format!(
                "<p>Recibimos una solicitud para restablecer tu contraseña.</p><p><a href=\"{enlace}\">Restablecer contraseña</a> (válido 30 minutos).</p><p>Si no fuiste tú, ignora este correo.</p>"
            )),
        };
        let email = Arc::clone(&self.email);
        let envio = tokio::task::spawn_blocking(move || email.enviar(&mensaje)).await;
        if !matches!(envio, Ok(Ok(()))) {
            // No se propaga: la respuesta debe ser idéntica exista o no la
            // cuenta. Se invalida el token para no dejar un enlace que nunca
            // se entregó y no se registra ni el correo ni el token (R-OB-1).
            crate::logs::error(
                "identity.password-reset",
                crate::campos! { "msg": "email_reset_fallido" },
            );
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            sqlx::query(
                "UPDATE identity.password_reset_tokens SET usado_en = NOW()
                  WHERE token_hash = $1",
            )
            .bind(&token_hash)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
        }
        Ok(())
    }

    pub async fn confirmar(&self, token: &str, nueva: &str) -> Result<(), ApiError> {
        if nueva.chars().count() < 8 {
            return Err(ApiError::dominio(
                "CONTRASENA_DEBIL",
                "La nueva contrasena debe tener al menos 8 caracteres.",
            ));
        }
        let token_hash = hash_token(token.trim());
        let hash = hash_contrasena(nueva)?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT usuario_id, expira_en, usado_en
               FROM identity.password_reset_tokens
              WHERE token_hash = $1
              FOR UPDATE",
        )
        .bind(&token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let usuario_id = validar_token(fila)?;

        sqlx::query("UPDATE identity.usuarios SET contrasena_hash = $1 WHERE id = $2::uuid")
            .bind(hash)
            .bind(&usuario_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.password_reset_tokens SET usado_en = NOW()
              WHERE usuario_id = $1::uuid AND usado_en IS NULL",
        )
        .bind(&usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        // Cambiar la contraseña invalida las sesiones abiertas (R-GW-4, ADR-18).
        self.sesiones.revocar_todas(&usuario_id).await?;
        Ok(())
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
        assert_eq!(primero.len(), 43);
        assert!(primero
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn el_hash_es_sha256_hexadecimal_y_estable() {
        let hash = hash_token("token");
        assert_eq!(hash.len(), 64);
        assert_eq!(hash, hash_token("token"));
        assert_ne!(hash, hash_token("otro"));
    }
}
