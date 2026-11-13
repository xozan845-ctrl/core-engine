//! Clientes internos (servicio→servicio) con timeout y fallo controlado.
//!
//! Resolución de datos por las APIs internas (ADR-0002/ADR-0003):
//! - orders: detalle y comprador/vendedor de la orden.
//! - identity: nombre y correo de cada usuario.
//!
//! No se leen esquemas ajenos. Un 404 se trata como "no disponible" (no se
//! reintenta); un fallo de red o 5xx se propaga para que Rabbit reintente/DLQ.

use crate::errors::ErrorNotificacion;
use crate::tipos::{OrdenExterna, UsuarioExterno};
use async_trait::async_trait;
use std::time::Duration;

/// UUID canónico (misma regla que `esUuid` de `@core/shared`).
pub fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    bytes.iter().enumerate().all(|(indice, byte)| match indice {
        8 | 13 | 18 | 23 => *byte == b'-',
        _ => byte.is_ascii_hexdigit(),
    })
}

#[async_trait]
pub trait Clientes: Send + Sync {
    async fn obtener_orden(
        &self,
        order_id: &str,
    ) -> Result<Option<OrdenExterna>, ErrorNotificacion>;
    async fn obtener_usuario(
        &self,
        usuario_id: &str,
    ) -> Result<Option<UsuarioExterno>, ErrorNotificacion>;
}

pub struct ClientesHttp {
    orders_url: String,
    identity_url: String,
    clave_interna: String,
    cliente: reqwest::Client,
}

impl ClientesHttp {
    pub fn nueva(
        orders_url: String,
        identity_url: String,
        clave_interna: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            orders_url: orders_url.trim_end_matches('/').to_string(),
            identity_url: identity_url.trim_end_matches('/').to_string(),
            clave_interna: clave_interna.unwrap_or_default(),
            cliente,
        }
    }

    async fn pedir<T>(&self, url: &str, servicio: &str) -> Result<Option<T>, ErrorNotificacion>
    where
        T: serde::de::DeserializeOwned,
    {
        let respuesta = self
            .cliente
            .get(url)
            .header("x-internal-key", &self.clave_interna)
            .send()
            .await
            .map_err(|_| ErrorNotificacion::Cliente(format!("No se pudo consultar {servicio}.")))?;
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !respuesta.status().is_success() {
            return Err(ErrorNotificacion::Cliente(format!(
                "{servicio} respondió {}.",
                respuesta.status().as_u16()
            )));
        }
        let cuerpo = respuesta.json::<T>().await.map_err(|error| {
            ErrorNotificacion::Cliente(format!(
                "{servicio} respondió una respuesta invalida: {error}"
            ))
        })?;
        Ok(Some(cuerpo))
    }
}

#[async_trait]
impl Clientes for ClientesHttp {
    async fn obtener_orden(
        &self,
        order_id: &str,
    ) -> Result<Option<OrdenExterna>, ErrorNotificacion> {
        if !es_uuid(order_id) {
            return Ok(None);
        }
        self.pedir(
            &format!(
                "{}/internal/orders/{}",
                self.orders_url,
                urlencoding(order_id)
            ),
            "orders",
        )
        .await
    }

    async fn obtener_usuario(
        &self,
        usuario_id: &str,
    ) -> Result<Option<UsuarioExterno>, ErrorNotificacion> {
        if !es_uuid(usuario_id) {
            return Ok(None);
        }
        self.pedir(
            &format!(
                "{}/internal/usuarios/{}",
                self.identity_url,
                urlencoding(usuario_id)
            ),
            "identity",
        )
        .await
    }
}

/// Equivalente mínimo a `encodeURIComponent` para IDs ya validados como UUID:
/// solo conserva caracteres seguros y escapa el resto en UTF-8.
fn urlencoding(valor: &str) -> String {
    let mut salida = String::new();
    for byte in valor.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => salida.push(byte as char),
            _ => salida.push_str(&format!("%{byte:02X}")),
        }
    }
    salida
}

/// Implementación para tests: las APIs internas no están disponibles.
pub struct ClientesNoDisponibles;

#[async_trait]
impl Clientes for ClientesNoDisponibles {
    async fn obtener_orden(
        &self,
        _order_id: &str,
    ) -> Result<Option<OrdenExterna>, ErrorNotificacion> {
        Err(ErrorNotificacion::Cliente(
            "No se pudo consultar orders.".to_string(),
        ))
    }

    async fn obtener_usuario(
        &self,
        _usuario_id: &str,
    ) -> Result<Option<UsuarioExterno>, ErrorNotificacion> {
        Err(ErrorNotificacion::Cliente(
            "No se pudo consultar identity.".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct Espia {
        clave: std::sync::Arc<Mutex<Option<String>>>,
        ruta: std::sync::Arc<Mutex<Option<String>>>,
    }

    async fn servidor_ok(espia: Espia) -> String {
        let app = Router::new()
            .route(
                "/internal/orders/{id}",
                get(
                    |State(espia): State<Espia>, Path(id): Path<String>, headers: HeaderMap| async move {
                        *espia.clave.lock().expect("lock") = headers
                            .get("x-internal-key")
                            .and_then(|valor| valor.to_str().ok())
                            .map(str::to_string);
                        *espia.ruta.lock().expect("lock") = Some(format!("/internal/orders/{id}"));
                        Json(json!({
                            "id": id,
                            "comprador_id": "11111111-1111-1111-1111-111111111111",
                            "vendedor_id": "22222222-2222-2222-2222-222222222222",
                            "estado": "pagada",
                            "total_cents": 115000,
                            "moneda": "NIO",
                        }))
                    },
                ),
            )
            .route(
                "/internal/usuarios/{id}",
                get(|Path(id): Path<String>| async move {
                    Json(json!({ "id": id, "nombre": "Ana", "correo": "ana@core.test" }))
                }),
            )
            .with_state(espia);
        arrancar(app).await
    }

    async fn servidor_status(status: StatusCode) -> String {
        let app = Router::new().route(
            "/internal/orders/{id}",
            get(move || async move { (status, Json(json!({}))) }),
        );
        arrancar(app).await
    }

    async fn arrancar(app: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let direccion = listener.local_addr().expect("direccion");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("servidor");
        });
        format!("http://{direccion}")
    }

    fn espia() -> Espia {
        Espia {
            clave: std::sync::Arc::new(Mutex::new(None)),
            ruta: std::sync::Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn debe_validar_uuids_y_no_consultar_ids_invalidos() {
        assert!(es_uuid("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"));
        assert!(!es_uuid("../../etc/passwd"));
        assert!(!es_uuid("otro"));
        assert!(!es_uuid(""));
    }

    #[test]
    fn debe_codificar_solo_lo_necesario() {
        assert_eq!(
            urlencoding("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        );
        assert_eq!(urlencoding("a/b"), "a%2Fb");
    }

    #[tokio::test]
    async fn debe_consultar_la_orden_con_la_clave_y_sin_doble_barra() {
        let espia = espia();
        let base = servidor_ok(espia.clone()).await;
        let clientes = ClientesHttp::nueva(
            format!("{base}/"),
            base,
            Some("clave-test".to_string()),
            1000,
        );
        let orden = clientes
            .obtener_orden("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .await
            .expect("orden")
            .expect("disponible");
        assert_eq!(orden.total_cents, 115000);
        assert_eq!(
            espia.clave.lock().expect("lock").as_deref(),
            Some("clave-test")
        );
        assert_eq!(
            espia.ruta.lock().expect("lock").as_deref(),
            Some("/internal/orders/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        );
    }

    #[tokio::test]
    async fn debe_consultar_el_usuario_por_identity() {
        let base = servidor_ok(espia()).await;
        let clientes = ClientesHttp::nueva(base.clone(), base, None, 1000);
        let usuario = clientes
            .obtener_usuario("11111111-1111-1111-1111-111111111111")
            .await
            .expect("usuario")
            .expect("disponible");
        assert_eq!(usuario.correo, "ana@core.test");
    }

    #[tokio::test]
    async fn un_404_devuelve_none_y_un_5xx_se_propaga() {
        let base = servidor_status(StatusCode::NOT_FOUND).await;
        let clientes = ClientesHttp::nueva(base.clone(), base, None, 1000);
        assert!(clientes
            .obtener_orden("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .await
            .expect("404")
            .is_none());

        let base = servidor_status(StatusCode::SERVICE_UNAVAILABLE).await;
        let clientes = ClientesHttp::nueva(base.clone(), base, None, 1000);
        let error = clientes
            .obtener_orden("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .await
            .expect_err("503");
        assert_eq!(error.mensaje(), "orders respondió 503.");
    }

    #[tokio::test]
    async fn un_fallo_de_red_se_propaga_para_reintentar() {
        let clientes = ClientesHttp::nueva(
            "http://127.0.0.1:1".to_string(),
            "http://127.0.0.1:1".to_string(),
            None,
            100,
        );
        let error = clientes
            .obtener_usuario("11111111-1111-1111-1111-111111111111")
            .await
            .expect_err("red");
        assert_eq!(error.mensaje(), "No se pudo consultar identity.");
    }

    #[tokio::test]
    async fn debe_fallar_cerrado_sin_servicios_internos() {
        assert!(ClientesNoDisponibles
            .obtener_orden("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .await
            .is_err());
        assert!(ClientesNoDisponibles
            .obtener_usuario("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn no_consulta_ids_que_no_son_uuid() {
        let espia = espia();
        let base = servidor_ok(espia.clone()).await;
        let clientes = ClientesHttp::nueva(base.clone(), base, None, 1000);
        assert!(clientes
            .obtener_orden("../../etc/passwd")
            .await
            .expect("orden")
            .is_none());
        assert!(clientes
            .obtener_usuario("otro")
            .await
            .expect("usuario")
            .is_none());
        assert!(espia.ruta.lock().expect("lock").is_none());
    }
}
