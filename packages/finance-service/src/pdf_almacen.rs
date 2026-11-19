//! Cliente interno de `storage-service` para persistir PDFs fuera de la base.
//!
//! Los bytes se envían en base64 al endpoint interno y se leen por id. Si el
//! almacenamiento no está disponible, `Facturacion` cae a `bytea` en Postgres.

use base64::Engine;
use serde_json::json;
use std::time::Duration;

pub struct PdfAlmacen {
    url: String,
    clave: String,
    cliente: reqwest::Client,
}

impl PdfAlmacen {
    pub fn nueva(url: impl Into<String>, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url: url.into().trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
            cliente: reqwest::Client::builder()
                .timeout(Duration::from_millis(timeout_ms.max(1)))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub async fn subir(
        &self,
        empresa_id: &str,
        nombre: &str,
        bytes: &[u8],
    ) -> Result<String, String> {
        let cuerpo = json!({
            "tenant_id": empresa_id,
            "nombre": nombre,
            "content_type": "application/pdf",
            "contenido_base64": base64::engine::general_purpose::STANDARD.encode(bytes),
            "publico": false,
            "descripcion": "PDF de comprobante fiscal",
        });
        let respuesta = self
            .cliente
            .post(format!("{}/internal/storage/objetos", self.url))
            .header("x-internal-key", &self.clave)
            .json(&cuerpo)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !respuesta.status().is_success() {
            return Err(format!("storage respondio {}", respuesta.status()));
        }
        let valor: serde_json::Value = respuesta.json().await.map_err(|error| error.to_string())?;
        valor
            .get("id")
            .or_else(|| valor.get("objeto_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "storage no devolvio el id del objeto".to_string())
    }

    pub async fn leer(&self, empresa_id: &str, objeto_id: &str) -> Result<Vec<u8>, String> {
        let respuesta = self
            .cliente
            .get(format!(
                "{}/internal/storage/objetos/{objeto_id}/contenido",
                self.url
            ))
            .query(&[("tenant_id", empresa_id)])
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !respuesta.status().is_success() {
            return Err(format!("storage respondio {}", respuesta.status()));
        }
        respuesta
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|error| error.to_string())
    }
}
