//! Canales externos de mensajería: WhatsApp Cloud API y SMS HTTP genérico.

use serde_json::json;
use std::time::Duration;

pub struct WhatsApp {
    url: String,
    token: String,
    cliente: reqwest::Client,
}

impl WhatsApp {
    pub fn nueva(phone_id: String, token: String, timeout_ms: u64) -> Self {
        Self {
            url: format!("https://graph.facebook.com/v20.0/{phone_id}/messages"),
            token,
            cliente: reqwest::Client::builder()
                .timeout(Duration::from_millis(timeout_ms.max(1)))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub async fn enviar(&self, telefono: &str, texto: &str) -> Result<(), String> {
        let respuesta = self
            .cliente
            .post(&self.url)
            .bearer_auth(&self.token)
            .json(&json!({
                "messaging_product": "whatsapp",
                "to": telefono,
                "type": "text",
                "text": { "body": texto },
            }))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !respuesta.status().is_success() {
            return Err(format!("whatsapp respondio {}", respuesta.status()));
        }
        Ok(())
    }
}

pub struct Sms {
    url: String,
    clave: Option<String>,
    cliente: reqwest::Client,
}

impl Sms {
    pub fn nuevo(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave,
            cliente: reqwest::Client::builder()
                .timeout(Duration::from_millis(timeout_ms.max(1)))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub async fn enviar(&self, telefono: &str, texto: &str) -> Result<(), String> {
        let mut peticion = self.cliente.post(&self.url).json(&json!({
            "to": telefono,
            "message": texto,
        }));
        if let Some(clave) = &self.clave {
            peticion = peticion.bearer_auth(clave);
        }
        let respuesta = peticion.send().await.map_err(|error| error.to_string())?;
        if !respuesta.status().is_success() {
            return Err(format!("sms respondio {}", respuesta.status()));
        }
        Ok(())
    }
}
