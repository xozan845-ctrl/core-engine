//! Cliente de la API interna de `stores-service` para validar tiendas.
//!
//! Réplica de `obtenerTienda`/`tiendasPorIds` de `@core/shared`: timeout corto,
//! `x-internal-key`, 404 = no encontrada y error de red = servicio caído.

use crate::errors::ApiError;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Tienda {
    pub id: String,
    pub nombre: String,
    pub empresa_id: String,
    #[serde(default)]
    pub vendedor_id: Option<String>,
    #[serde(default)]
    pub participacion_porcentaje: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Stores {
    url: String,
    clave: String,
    timeout: Duration,
}

impl Stores {
    pub fn nueva(url: impl Into<String>, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url: url.into().trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(if timeout_ms > 0 { timeout_ms } else { 3000 }),
        }
    }

    fn cliente(&self) -> Result<reqwest::Client, ApiError> {
        reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ApiError::servicio_no_disponible("stores-service"))
    }

    /// `None` si no existe; 503 si stores no responde.
    pub async fn obtener(&self, tienda_id: &str) -> Result<Option<Tienda>, ApiError> {
        let respuesta = self
            .cliente()?
            .get(format!("{}/internal/tiendas/{tienda_id}", self.url))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("stores-service"))?;
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !respuesta.status().is_success() {
            return Err(ApiError::servicio_no_disponible("stores-service"));
        }
        respuesta
            .json::<Tienda>()
            .await
            .map(Some)
            .map_err(|_| ApiError::servicio_no_disponible("stores-service"))
    }

    /// `None` = stores no disponible (degradación controlada del listado).
    pub async fn por_ids(&self, ids: &[String]) -> Result<Option<Vec<Tienda>>, ApiError> {
        if ids.is_empty() {
            return Ok(Some(Vec::new()));
        }
        let respuesta = match self
            .cliente()?
            .get(format!("{}/internal/tiendas", self.url))
            .query(&[("ids", ids.join(","))])
            .header("x-internal-key", &self.clave)
            .send()
            .await
        {
            Ok(respuesta) if respuesta.status().is_success() => respuesta,
            _ => return Ok(None),
        };
        #[derive(Deserialize)]
        struct Respuesta {
            #[serde(default)]
            tiendas: Vec<Tienda>,
        }
        match respuesta.json::<Respuesta>().await {
            Ok(cuerpo) => Ok(Some(cuerpo.tiendas)),
            Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_normalizar_la_url_y_el_timeout() {
        let stores = Stores::nueva("http://stores-service:3003/", None, 0);
        assert_eq!(stores.url, "http://stores-service:3003");
        assert_eq!(stores.timeout, Duration::from_millis(3000));
    }

    #[test]
    fn debe_deserializar_una_tienda_externa() {
        let tienda: Tienda = serde_json::from_str(
            r#"{"id":"t-1","nombre":"Tienda","empresa_id":"e-1","vendedor_id":null}"#,
        )
        .expect("tienda");
        assert_eq!(tienda.nombre, "Tienda");
        assert!(tienda.vendedor_id.is_none());
    }
}
