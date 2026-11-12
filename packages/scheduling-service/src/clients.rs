//! Clientes internos (servicio→servicio) con timeout y fallo controlado.

use async_trait::async_trait;
use std::time::Duration;

/// Consulta de pertenencia de una parte a la empresa (identity, superficie
/// interna). `Ok(false)` = 404; `Err(())` = servicio no disponible.
#[async_trait]
pub trait Partes: Send + Sync {
    async fn pertenece(&self, empresa_id: &str, parte_id: &str) -> Result<bool, ()>;
}

pub struct PartesHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl PartesHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl Partes for PartesHttp {
    async fn pertenece(&self, empresa_id: &str, parte_id: &str) -> Result<bool, ()> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ())?;
        let respuesta = cliente
            .get(format!("{}/internal/partes/{parte_id}", self.url))
            .query(&[("empresa_id", empresa_id)])
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .map_err(|_| ())?;
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }
        if !respuesta.status().is_success() {
            return Err(());
        }
        respuesta
            .json::<serde_json::Value>()
            .await
            .map(|_| true)
            .map_err(|_| ())
    }
}

/// Implementación para tests: identity no disponible.
pub struct PartesNoDisponible;

#[async_trait]
impl Partes for PartesNoDisponible {
    async fn pertenece(&self, _empresa_id: &str, _parte_id: &str) -> Result<bool, ()> {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn debe_fallar_cerrado_sin_identity() {
        assert_eq!(PartesNoDisponible.pertenece("e-1", "p-1").await, Err(()));
    }
}
