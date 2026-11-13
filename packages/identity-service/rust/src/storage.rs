//! Cliente de la API interna de `storage-service` para validar medios.
//!
//! Réplica de `media-background.ts` en lo que identity necesita para avatares:
//! el objeto debe existir, estar activo, ser público, del tenant y ser imagen.

use crate::errors::ApiError;
use axum::http::StatusCode;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ObjetoStorage {
    pub tenant_id: Option<String>,
    pub estado: Option<String>,
    pub publico: Option<bool>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Storage {
    url: String,
    clave: String,
    timeout: Duration,
}

impl Storage {
    pub fn nueva(url: impl Into<String>, clave: Option<String>) -> Self {
        Self {
            url: url.into().trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(4000),
        }
    }

    pub async fn objeto(
        &self,
        tenant_id: &str,
        objeto_id: &str,
    ) -> Result<ObjetoStorage, ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| no_verificable())?;
        let respuesta = cliente
            .get(format!("{}/internal/storage/objetos/{objeto_id}", self.url))
            .query(&[("tenant_id", tenant_id)])
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .map_err(|_| no_verificable())?;
        if respuesta.status() == StatusCode::NOT_FOUND {
            return Err(ApiError::no_encontrado("Avatar", objeto_id));
        }
        if !respuesta.status().is_success() {
            return Err(no_verificable());
        }
        respuesta
            .json::<ObjetoStorage>()
            .await
            .map_err(|_| no_verificable())
    }

    /// Valida que el objeto sea una imagen pública y activa del tenant.
    pub async fn validar_avatar(&self, tenant_id: &str, objeto_id: &str) -> Result<(), ApiError> {
        let objeto = self.objeto(tenant_id, objeto_id).await?;
        let es_imagen = objeto
            .content_type
            .as_deref()
            .is_some_and(|tipo| tipo.starts_with("image/"));
        if objeto.tenant_id.as_deref() != Some(tenant_id)
            || objeto.estado.as_deref() != Some("activo")
            || objeto.publico != Some(true)
            || !es_imagen
        {
            return Err(ApiError::dominio(
                "AVATAR_INVALIDO",
                "El avatar debe ser una imagen pública y activa del tenant.",
            ));
        }
        Ok(())
    }
}

fn no_verificable() -> ApiError {
    ApiError::dominio("AVATAR_NO_VERIFICABLE", "No se pudo verificar el avatar.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_normalizar_la_url_del_servicio() {
        let storage = Storage::nueva("http://storage-service:3010/", None);
        assert_eq!(storage.url, "http://storage-service:3010");
    }

    #[test]
    fn debe_conservar_un_objeto_publico_valido() {
        let objeto = ObjetoStorage {
            tenant_id: Some("t-1".into()),
            estado: Some("activo".into()),
            publico: Some(true),
            content_type: Some("image/png".into()),
        };
        assert_eq!(objeto.tenant_id.as_deref(), Some("t-1"));
        assert!(objeto
            .content_type
            .as_deref()
            .is_some_and(|tipo| tipo.starts_with("image/")));
    }
}
