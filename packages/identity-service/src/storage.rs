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

impl Storage {
    /// Valida un fondo o poster contra storage-service y deriva `imagen|video`.
    /// El poster solo admite imágenes; el fondo admite imágenes y videos.
    pub async fn validar_media(
        &self,
        tenant_id: &str,
        objeto_id: &str,
        uso: &str,
    ) -> Result<String, ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| no_verificable_media())?;
        let respuesta = cliente
            .get(format!("{}/internal/storage/objetos/{objeto_id}", self.url))
            .query(&[("tenant_id", tenant_id)])
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .map_err(|_| no_verificable_media())?;
        if respuesta.status() == StatusCode::NOT_FOUND {
            return Err(ApiError::no_encontrado(
                if uso == "poster" { "Poster" } else { "Fondo" },
                objeto_id,
            ));
        }
        if !respuesta.status().is_success() {
            return Err(no_verificable_media());
        }
        let objeto = respuesta
            .json::<ObjetoStorage>()
            .await
            .map_err(|_| no_verificable_media())?;
        let tipo = objeto.content_type.as_deref().unwrap_or("").to_lowercase();
        let permitidos: Vec<&str> = if uso == "poster" {
            crate::dominio::media::BACKGROUND_IMAGENES.to_vec()
        } else {
            let mut permitidos = crate::dominio::media::BACKGROUND_IMAGENES.to_vec();
            permitidos.extend(crate::dominio::media::BACKGROUND_VIDEOS);
            permitidos
        };
        if objeto.tenant_id.as_deref() != Some(tenant_id)
            || objeto.estado.as_deref() != Some("activo")
            || objeto.publico != Some(true)
            || !permitidos.contains(&tipo.as_str())
        {
            return Err(ApiError::dominio(
                crate::dominio::media::ERROR_INVALIDO,
                if uso == "poster" {
                    "El poster debe ser una imagen pública y activa del mismo tenant."
                } else {
                    "El fondo debe ser una imagen o video público y activo del mismo tenant."
                },
            ));
        }
        Ok(if tipo.starts_with("video/") {
            "video".to_string()
        } else {
            "imagen".to_string()
        })
    }
}

fn no_verificable_media() -> ApiError {
    ApiError::dominio(
        crate::dominio::media::ERROR_NO_VERIFICABLE,
        "No se pudo verificar la media del fondo.",
    )
}

impl Storage {
    /// El logo debe ser una imagen pública y activa de la empresa.
    pub async fn validar_logo(&self, tenant_id: &str, objeto_id: &str) -> Result<(), ApiError> {
        let objeto = match self.objeto(tenant_id, objeto_id).await {
            Ok(objeto) => objeto,
            Err(error) if error.codigo == "NO_ENCONTRADO" => {
                return Err(ApiError::no_encontrado("Logo", objeto_id))
            }
            Err(_) => {
                return Err(ApiError::dominio(
                    "LOGO_NO_VERIFICABLE",
                    "No se pudo verificar el logo.",
                ))
            }
        };
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
                "LOGO_INVALIDO",
                "El logo debe ser una imagen pública y activa de la empresa.",
            ));
        }
        Ok(())
    }
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
