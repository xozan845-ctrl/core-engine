//! Adaptador S3/MinIO (ADR-24).
//!
//! El dominio depende de `AlmacenObjetos`; cambiar de proveedor S3-compatible
//! solo toca esta implementación. Los bytes no pasan por el servicio salvo en
//! el endpoint público de media (streaming/Range).

use crate::config::Config;
use async_trait::async_trait;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjetoEnAlmacen {
    pub tamano_bytes: i64,
    pub content_type: Option<String>,
    pub etag: Option<String>,
}

pub struct ObjetoLeido {
    pub cuerpo: ByteStream,
    pub tamano_bytes: i64,
    pub content_type: Option<String>,
    pub etag: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AlmacenError {
    #[error("objeto no encontrado")]
    NoEncontrado,
    #[error("fallo del almacen: {0}")]
    Operacion(String),
}

#[async_trait]
pub trait AlmacenObjetos: Send + Sync {
    async fn asegurar_bucket(&self, bucket: &str) -> Result<(), AlmacenError>;
    async fn existe_bucket(&self, bucket: &str) -> Result<bool, AlmacenError>;
    async fn url_subida(
        &self,
        bucket: &str,
        objeto: &str,
        content_type: &str,
        expira_segundos: u64,
    ) -> Result<String, AlmacenError>;
    async fn url_descarga(
        &self,
        bucket: &str,
        objeto: &str,
        expira_segundos: u64,
    ) -> Result<String, AlmacenError>;
    async fn estadisticas(
        &self,
        bucket: &str,
        objeto: &str,
    ) -> Result<Option<ObjetoEnAlmacen>, AlmacenError>;
    async fn leer(&self, bucket: &str, objeto: &str) -> Result<Option<ObjetoLeido>, AlmacenError>;
    async fn leer_rango(
        &self,
        bucket: &str,
        objeto: &str,
        inicio: i64,
        fin: i64,
    ) -> Result<Option<ObjetoLeido>, AlmacenError>;
    async fn eliminar(&self, bucket: &str, objeto: &str) -> Result<(), AlmacenError>;
}

pub struct AlmacenS3 {
    cliente: Client,
    cliente_firma: Client,
    asegurados: Arc<Mutex<HashSet<String>>>,
}

impl AlmacenS3 {
    pub fn nueva(config: &Config) -> Self {
        let credenciales = Credentials::new(
            config.minio_access_key.clone().unwrap_or_default(),
            config.minio_secret_key.clone().unwrap_or_default(),
            None,
            None,
            "storage-service",
        );
        let construir = |endpoint: String| {
            let cfg = aws_sdk_s3::Config::builder()
                .behavior_version(BehaviorVersion::latest())
                .credentials_provider(credenciales.clone())
                .region(Region::new(config.minio_region.clone()))
                .endpoint_url(endpoint)
                .force_path_style(true)
                .build();
            Client::from_conf(cfg)
        };
        Self {
            cliente: construir(config.endpoint_url()),
            // Las URLs prefirmadas llevan el host firmado: deben apuntar al
            // endpoint que resuelve el cliente final, no al nombre interno.
            cliente_firma: construir(config.public_endpoint_url()),
            asegurados: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    fn presign(expira_segundos: u64) -> Result<PresigningConfig, AlmacenError> {
        PresigningConfig::expires_in(Duration::from_secs(expira_segundos.max(1)))
            .map_err(|error| AlmacenError::Operacion(error.to_string()))
    }

    fn error_operacion(error: impl std::fmt::Display) -> AlmacenError {
        AlmacenError::Operacion(error.to_string())
    }
}

fn es_no_encontrado<E>(error: &aws_sdk_s3::error::SdkError<E>) -> bool
where
    E: std::error::Error + 'static,
{
    match error {
        aws_sdk_s3::error::SdkError::ServiceError(contexto) => {
            contexto.raw().status().as_u16() == 404
        }
        _ => false,
    }
}

#[async_trait]
impl AlmacenObjetos for AlmacenS3 {
    async fn asegurar_bucket(&self, bucket: &str) -> Result<(), AlmacenError> {
        {
            let asegurados = self.asegurados.lock().await;
            if asegurados.contains(bucket) {
                return Ok(());
            }
        }
        let existe = self
            .cliente
            .head_bucket()
            .bucket(bucket)
            .send()
            .await
            .is_ok();
        if !existe {
            self.cliente
                .create_bucket()
                .bucket(bucket)
                .send()
                .await
                .map_err(Self::error_operacion)?;
        }
        self.asegurados.lock().await.insert(bucket.to_string());
        Ok(())
    }

    async fn existe_bucket(&self, bucket: &str) -> Result<bool, AlmacenError> {
        match self.cliente.head_bucket().bucket(bucket).send().await {
            Ok(_) => Ok(true),
            Err(error) if es_no_encontrado(&error) => Ok(false),
            Err(error) => Err(Self::error_operacion(error)),
        }
    }

    async fn url_subida(
        &self,
        bucket: &str,
        objeto: &str,
        content_type: &str,
        expira_segundos: u64,
    ) -> Result<String, AlmacenError> {
        let presign = Self::presign(expira_segundos)?;
        let request = self
            .cliente_firma
            .put_object()
            .bucket(bucket)
            .key(objeto)
            .content_type(content_type)
            .presigned(presign)
            .await
            .map_err(Self::error_operacion)?;
        Ok(request.uri().to_string())
    }

    async fn url_descarga(
        &self,
        bucket: &str,
        objeto: &str,
        expira_segundos: u64,
    ) -> Result<String, AlmacenError> {
        let presign = Self::presign(expira_segundos)?;
        let request = self
            .cliente_firma
            .get_object()
            .bucket(bucket)
            .key(objeto)
            .presigned(presign)
            .await
            .map_err(Self::error_operacion)?;
        Ok(request.uri().to_string())
    }

    async fn estadisticas(
        &self,
        bucket: &str,
        objeto: &str,
    ) -> Result<Option<ObjetoEnAlmacen>, AlmacenError> {
        match self
            .cliente
            .head_object()
            .bucket(bucket)
            .key(objeto)
            .send()
            .await
        {
            Ok(salida) => Ok(Some(ObjetoEnAlmacen {
                tamano_bytes: salida.content_length().unwrap_or(0),
                content_type: salida.content_type().map(str::to_string),
                etag: salida.e_tag().map(str::to_string),
            })),
            Err(error) if es_no_encontrado(&error) => Ok(None),
            Err(error) => Err(Self::error_operacion(error)),
        }
    }

    async fn leer(&self, bucket: &str, objeto: &str) -> Result<Option<ObjetoLeido>, AlmacenError> {
        self.leer_interno(bucket, objeto, None).await
    }

    async fn leer_rango(
        &self,
        bucket: &str,
        objeto: &str,
        inicio: i64,
        fin: i64,
    ) -> Result<Option<ObjetoLeido>, AlmacenError> {
        self.leer_interno(bucket, objeto, Some(format!("bytes={inicio}-{fin}")))
            .await
    }

    async fn eliminar(&self, bucket: &str, objeto: &str) -> Result<(), AlmacenError> {
        self.cliente
            .delete_object()
            .bucket(bucket)
            .key(objeto)
            .send()
            .await
            .map_err(Self::error_operacion)?;
        Ok(())
    }
}

impl AlmacenS3 {
    async fn leer_interno(
        &self,
        bucket: &str,
        objeto: &str,
        rango: Option<String>,
    ) -> Result<Option<ObjetoLeido>, AlmacenError> {
        let mut peticion = self.cliente.get_object().bucket(bucket).key(objeto);
        if let Some(rango) = rango {
            peticion = peticion.range(rango);
        }
        match peticion.send().await {
            Ok(salida) => {
                let tamano_bytes = salida.content_length().unwrap_or(0);
                let content_type = salida.content_type().map(str::to_string);
                let etag = salida.e_tag().map(str::to_string);
                Ok(Some(ObjetoLeido {
                    cuerpo: salida.body,
                    tamano_bytes,
                    content_type,
                    etag,
                }))
            }
            Err(error) if es_no_encontrado(&error) => Ok(None),
            Err(error) => Err(Self::error_operacion(error)),
        }
    }
}

/// Implementación para tests y arranque sin credenciales (swagger/health).
#[allow(dead_code)]
pub struct AlmacenNoDisponible;

#[async_trait]
impl AlmacenObjetos for AlmacenNoDisponible {
    async fn asegurar_bucket(&self, _: &str) -> Result<(), AlmacenError> {
        Err(AlmacenError::Operacion("almacen no configurado".into()))
    }
    async fn existe_bucket(&self, _: &str) -> Result<bool, AlmacenError> {
        Ok(false)
    }
    async fn url_subida(&self, _: &str, _: &str, _: &str, _: u64) -> Result<String, AlmacenError> {
        Err(AlmacenError::Operacion("almacen no configurado".into()))
    }
    async fn url_descarga(&self, _: &str, _: &str, _: u64) -> Result<String, AlmacenError> {
        Err(AlmacenError::Operacion("almacen no configurado".into()))
    }
    async fn estadisticas(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<ObjetoEnAlmacen>, AlmacenError> {
        Ok(None)
    }
    async fn leer(&self, _: &str, _: &str) -> Result<Option<ObjetoLeido>, AlmacenError> {
        Ok(None)
    }
    async fn leer_rango(
        &self,
        _: &str,
        _: &str,
        _: i64,
        _: i64,
    ) -> Result<Option<ObjetoLeido>, AlmacenError> {
        Ok(None)
    }
    async fn eliminar(&self, _: &str, _: &str) -> Result<(), AlmacenError> {
        Ok(())
    }
}
