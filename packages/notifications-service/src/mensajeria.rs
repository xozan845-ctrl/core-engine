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
        Self::con_url(
            format!("https://graph.facebook.com/v20.0/{phone_id}/messages"),
            token,
            timeout_ms,
        )
    }

    /// Construye el cliente con una URL explícita (pruebas y proxies).
    pub fn con_url(url: String, token: String, timeout_ms: u64) -> Self {
        Self {
            url,
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::{Json, Router};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct Servidor {
        url: String,
        recibido: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    async fn servidor(status: StatusCode) -> Servidor {
        let recibido = Arc::new(Mutex::new(Vec::new()));
        let estado = Arc::clone(&recibido);
        let app = Router::new().route(
            "/enviar",
            post(move |Json(cuerpo): Json<serde_json::Value>| {
                let estado = Arc::clone(&estado);
                async move {
                    estado.lock().await.push(cuerpo);
                    status
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let url = format!("http://{}/enviar", listener.local_addr().expect("addr"));
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Servidor { url, recibido }
    }

    #[tokio::test]
    async fn whatsapp_envia_el_cuerpo_esperado() {
        let servidor = servidor(StatusCode::OK).await;
        let whatsapp = WhatsApp::con_url(servidor.url.clone(), "token".into(), 2000);
        whatsapp
            .enviar("+50588887777", "Pedido listo")
            .await
            .expect("envio");
        let cuerpos = servidor.recibido.lock().await;
        assert_eq!(cuerpos.len(), 1);
        assert_eq!(cuerpos[0]["messaging_product"], "whatsapp");
        assert_eq!(cuerpos[0]["to"], "+50588887777");
        assert_eq!(cuerpos[0]["text"]["body"], "Pedido listo");
    }

    #[tokio::test]
    async fn whatsapp_propaga_el_estado_no_exitoso() {
        let servidor = servidor(StatusCode::INTERNAL_SERVER_ERROR).await;
        let whatsapp = WhatsApp::con_url(servidor.url.clone(), "token".into(), 2000);
        let error = whatsapp
            .enviar("+50588887777", "Pedido listo")
            .await
            .expect_err("error");
        assert!(error.contains("500"));
    }

    #[tokio::test]
    async fn sms_envia_con_y_sin_clave() {
        let servidor = servidor(StatusCode::OK).await;
        let sms = Sms::nuevo(servidor.url.clone(), Some("clave".into()), 2000);
        sms.enviar("+50588887777", "Hola").await.expect("sms");
        let sms_sin_clave = Sms::nuevo(servidor.url.clone(), None, 2000);
        sms_sin_clave
            .enviar("+50588887777", "Hola")
            .await
            .expect("sms");
        let cuerpos = servidor.recibido.lock().await;
        assert_eq!(cuerpos.len(), 2);
        assert_eq!(cuerpos[1]["message"], "Hola");
    }

    #[tokio::test]
    async fn sms_propaga_el_estado_no_exitoso() {
        let servidor = servidor(StatusCode::BAD_GATEWAY).await;
        let sms = Sms::nuevo(servidor.url.clone(), None, 2000);
        let error = sms.enviar("+50588887777", "Hola").await.expect_err("error");
        assert!(error.contains("502"));
    }
}
