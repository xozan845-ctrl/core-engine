//! Adaptador de Tilopay (API v1) para la ACL `ProveedorPagos`.
//!
//! No contiene credenciales: lee nombres de variables de entorno desde la
//! configuración del proveedor (`orders.proveedores_pago.config`). La URL base
//! también es configurable y tiene un default oficial.

use crate::errors::ApiError;
use crate::pagos::{liquidacion_de, CargoExterno, ConsultaExterna, EstadoExterno, ProveedorPagos};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const BASE_URL_DEFAULT: &str = "https://app.tilopay.com";

fn tokens() -> &'static Mutex<HashMap<String, (String, Instant)>> {
    static TOKENS: OnceLock<Mutex<HashMap<String, (String, Instant)>>> = OnceLock::new();
    TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn requerido(config: &Value, clave: &str) -> Result<String, ApiError> {
    config
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            ApiError::dominio(
                "PROVEEDOR_PAGO_INCOMPLETO",
                format!("Tilopay requiere config.{clave}."),
            )
        })
}

fn variable(config: &Value, clave: &str, default: &str) -> Result<String, ApiError> {
    let nombre = config
        .get(clave)
        .and_then(Value::as_str)
        .filter(|valor| !valor.is_empty())
        .unwrap_or(default);
    std::env::var(nombre).map_err(|_| {
        ApiError::dominio(
            "PROVEEDOR_PAGO_INCOMPLETO",
            format!("Falta la variable de entorno {nombre} para Tilopay."),
        )
    })
}

pub struct Tilopay {
    http: Client,
    base: String,
    apiuser: String,
    password: String,
    key: String,
    callback_url: Option<String>,
    webhook_url: Option<String>,
    /// `test` exige que Tilopay reporte `environment=Test` en `consult`.
    ambiente: Option<String>,
    /// Unidad de los montos de liquidación: `major` (3.50) o `cents` (350).
    liquidacion_unidad: String,
}

impl Tilopay {
    pub fn nuevo(config: &Value) -> Result<Self, ApiError> {
        let base = config
            .get("base_url")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(BASE_URL_DEFAULT)
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base,
            apiuser: variable(config, "apiuser_env", "TILOPAY_APIUSER")?,
            password: variable(config, "password_env", "TILOPAY_PASSWORD")?,
            key: variable(config, "key_env", "TILOPAY_KEY")?,
            callback_url: config
                .get("callback_url")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .map(str::to_string),
            webhook_url: config
                .get("webhook_url")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .map(str::to_string),
            ambiente: config
                .get("ambiente")
                .and_then(Value::as_str)
                .map(|valor| valor.trim().to_ascii_lowercase())
                .filter(|valor| !valor.is_empty()),
            liquidacion_unidad: config
                .get("liquidacion_unidad")
                .and_then(Value::as_str)
                .map(|valor| valor.trim().to_ascii_lowercase())
                .filter(|valor| *valor == "cents")
                .unwrap_or_else(|| "major".to_string()),
        })
    }

    fn clave_cache(&self) -> String {
        format!("{}|{}", self.base, self.apiuser)
    }

    async fn token(&self) -> Result<String, ApiError> {
        let clave = self.clave_cache();
        if let Ok(cache) = tokens().lock() {
            if let Some((token, expira)) = cache.get(&clave) {
                if *expira > Instant::now() {
                    return Ok(token.clone());
                }
            }
        }
        let respuesta = self
            .http
            .post(format!("{}/api/v1/login", self.base))
            .json(&json!({ "apiuser": self.apiuser, "password": self.password }))
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio("PROVEEDOR_PAGO_NO_DISPONIBLE", "Tilopay no responde.")
            })?;
        let cuerpo: Value = respuesta.json().await.map_err(|_| {
            ApiError::dominio("PROVEEDOR_PAGO_ERROR", "Respuesta inválida de Tilopay.")
        })?;
        let token = cuerpo
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ApiError::dominio(
                    "PROVEEDOR_PAGO_ERROR",
                    "Tilopay no devolvió access_token; revisar credenciales.",
                )
            })?
            .to_string();
        let segundos = cuerpo
            .get("expires_in")
            .and_then(Value::as_u64)
            .unwrap_or(3600)
            .saturating_sub(60);
        if let Ok(mut cache) = tokens().lock() {
            cache.insert(
                clave,
                (
                    token.clone(),
                    Instant::now() + Duration::from_secs(segundos),
                ),
            );
        }
        Ok(token)
    }

    fn monto(monto_cents: i64) -> String {
        format!("{}.{:02}", monto_cents / 100, monto_cents.rem_euclid(100))
    }
}

#[async_trait]
impl ProveedorPagos for Tilopay {
    fn nombre(&self) -> String {
        "tilopay".to_string()
    }

    async fn crear_cargo(
        &self,
        order_id: &str,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        let token = self.token().await?;
        let mut cuerpo = json!({
            "key": self.key,
            "amount": Self::monto(monto_cents),
            "currency": moneda,
            "reference": order_id,
            "type": 1,
            "description": format!("Orden {order_id}"),
        });
        if let Some(callback) = &self.callback_url {
            cuerpo["callback_url"] = json!(callback);
        }
        if let Some(webhook) = &self.webhook_url {
            // El webhook de Tilopay no incluye un id interno fiable; se añade
            // el order_id a la URL para correlacionar sin confiar en el body.
            let separador = if webhook.contains('?') { '&' } else { '?' };
            cuerpo["webhook_url"] = json!(format!("{webhook}{separador}order_id={order_id}"));
        }
        let respuesta = self
            .http
            .post(format!("{}/api/v1/createLinkPayment", self.base))
            .bearer_auth(token)
            .json(&cuerpo)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio("PROVEEDOR_PAGO_NO_DISPONIBLE", "Tilopay no responde.")
            })?;
        let valor: Value = respuesta.json().await.map_err(|_| {
            ApiError::dominio("PROVEEDOR_PAGO_ERROR", "Respuesta inválida de Tilopay.")
        })?;
        if valor.get("type").and_then(Value::as_str) != Some("200") {
            return Err(ApiError::dominio(
                "PAGO_RECHAZADO",
                valor
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Tilopay rechazó el link de pago.")
                    .to_string(),
            ));
        }
        let url = valor
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ApiError::dominio(
                    "PROVEEDOR_PAGO_ERROR",
                    "Tilopay no devolvió el link de pago.",
                )
            })?
            .to_string();
        Ok(CargoExterno {
            referencia: order_id.to_string(),
            url: Some(url),
            liquidacion: liquidacion_de("tilopay", &valor, &self.liquidacion_unidad),
        })
    }

    async fn reembolsar(&self, referencia: &str, monto_cents: i64) -> Result<(), ApiError> {
        let token = self.token().await?;
        let respuesta = self
            .http
            .post(format!("{}/api/v1/processModification", self.base))
            .bearer_auth(token)
            .json(&json!({
                "orderNumber": referencia,
                "type": "2",
                "amount": Self::monto(monto_cents),
                "key": self.key,
            }))
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio("PROVEEDOR_PAGO_NO_DISPONIBLE", "Tilopay no responde.")
            })?;
        let valor: Value = respuesta.json().await.map_err(|_| {
            ApiError::dominio("PROVEEDOR_PAGO_ERROR", "Respuesta inválida de Tilopay.")
        })?;
        let aprobado = valor.get("type").and_then(Value::as_str) == Some("100")
            && valor.get("ReasonCode").and_then(Value::as_str) == Some("1101");
        if aprobado {
            Ok(())
        } else {
            Err(ApiError::dominio(
                "REEMBOLSO_RECHAZADO",
                valor
                    .get("ReasonCodeDescription")
                    .and_then(Value::as_str)
                    .unwrap_or("Tilopay rechazó el reembolso.")
                    .to_string(),
            ))
        }
    }

    async fn contracargar(
        &self,
        _referencia: &str,
        _monto_cents: i64,
        _motivo: &str,
    ) -> Result<(), ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_NO_SOPORTA_CONTRACARGO",
            "Tilopay gestiona contracargos desde su panel; no expone un cargo reverso por API.",
        ))
    }

    async fn consultar(
        &self,
        referencia: &str,
        datos: Option<&Value>,
    ) -> Result<EstadoExterno, ApiError> {
        Ok(self.consultar_detallado(referencia, datos).await?.estado)
    }

    async fn consultar_detallado(
        &self,
        referencia: &str,
        datos: Option<&Value>,
    ) -> Result<ConsultaExterna, ApiError> {
        let order_number = datos
            .and_then(|valor| valor.get("orderNumber"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(referencia);
        let token = self.token().await?;
        let respuesta = self
            .http
            .post(format!("{}/api/v1/consult", self.base))
            .bearer_auth(token)
            .json(&json!({ "key": self.key, "orderNumber": order_number }))
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio("PROVEEDOR_PAGO_NO_DISPONIBLE", "Tilopay no responde.")
            })?;
        let valor: Value = respuesta.json().await.map_err(|_| {
            ApiError::dominio("PROVEEDOR_PAGO_ERROR", "Respuesta inválida de Tilopay.")
        })?;
        if valor.get("type").and_then(Value::as_str) != Some("200") {
            return Ok(ConsultaExterna {
                estado: EstadoExterno::Pendiente,
                liquidacion: None,
            });
        }
        let fila = valor
            .get("response")
            .and_then(Value::as_array)
            .and_then(|filas| filas.first());
        if self.ambiente.as_deref() == Some("test") {
            if let Some(ambiente) = fila
                .and_then(|fila| fila.get("environment"))
                .and_then(Value::as_str)
                .filter(|valor| !valor.trim().is_empty())
            {
                if !ambiente.trim().eq_ignore_ascii_case("test") {
                    return Err(ApiError::dominio(
                        "TILOPAY_AMBIENTE_INESPERADO",
                        format!(
                            "Tilopay reporta environment={ambiente}; la configuración exige Test. No se confirma la orden."
                        ),
                    ));
                }
            }
        }
        let codigo = fila
            .and_then(|fila| fila.get("code"))
            .map(|codigo| {
                codigo
                    .as_str()
                    .map(str::to_string)
                    .or_else(|| codigo.as_i64().map(|numero| numero.to_string()))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let estado = match codigo.as_str() {
            "1" => EstadoExterno::Aprobado,
            "" => EstadoExterno::Pendiente,
            _ => EstadoExterno::Rechazado,
        };
        let liquidacion =
            fila.and_then(|fila| liquidacion_de("tilopay", fila, &self.liquidacion_unidad));
        Ok(ConsultaExterna {
            estado,
            liquidacion,
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::extract::State;
    use axum::http::Request;
    use axum::routing::post;
    use axum::{Json, Router};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Registro {
        peticiones: Arc<Mutex<Vec<(String, Value)>>>,
        login: Arc<AtomicUsize>,
        reembolso_ok: Arc<AtomicBool>,
        consulta_codigo: Arc<Mutex<String>>,
        ambiente: Arc<Mutex<String>>,
    }

    async fn responder(State(registro): State<Registro>, peticion: Request<Body>) -> Json<Value> {
        let ruta = peticion.uri().path().to_string();
        let bytes = to_bytes(peticion.into_body(), 1_000_000)
            .await
            .unwrap_or_default();
        let cuerpo: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        registro
            .peticiones
            .lock()
            .expect("registro")
            .push((ruta.clone(), cuerpo));
        let respuesta = match ruta.as_str() {
            "/api/v1/login" => {
                registro.login.fetch_add(1, Ordering::SeqCst);
                json!({"access_token": "tok-1", "expires_in": 3600})
            }
            "/api/v1/createLinkPayment" => {
                json!({"type": "200", "url": "https://pagos.example/link/1"})
            }
            "/api/v1/processModification" => {
                if registro.reembolso_ok.load(Ordering::SeqCst) {
                    json!({"type": "100", "ReasonCode": "1101"})
                } else {
                    json!({
                        "type": "100",
                        "ReasonCode": "1105",
                        "ReasonCodeDescription": "Fondos insuficientes"
                    })
                }
            }
            "/api/v1/consult" => json!({
                "type": "200",
                "response": [{
                    "code": registro.consulta_codigo.lock().expect("consulta").clone(),
                    "environment": registro.ambiente.lock().expect("ambiente").clone()
                }]
            }),
            _ => json!({"type": "999"}),
        };
        Json(respuesta)
    }

    async fn levantar(registro: Registro) -> String {
        let app = Router::new().fallback(post(responder)).with_state(registro);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("puerto");
        let direccion = format!("http://{}", listener.local_addr().expect("direccion"));
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("servidor mock");
        });
        direccion
    }

    fn config(base: &str, sufijo: &str) -> Value {
        let usuario = format!("TILOPAY_TEST_USER_{sufijo}");
        let clave = format!("TILOPAY_TEST_PASSWORD_{sufijo}");
        let llave = format!("TILOPAY_TEST_KEY_{sufijo}");
        std::env::set_var(&usuario, "usuario");
        std::env::set_var(&clave, "secreto");
        std::env::set_var(&llave, "llave");
        json!({
            "base_url": base,
            "apiuser_env": usuario,
            "password_env": clave,
            "key_env": llave,
            "webhook_url": "https://hooks.example/tilopay",
            "callback_url": "https://app.example/pago/retorno",
            "ambiente": "test",
        })
    }

    #[tokio::test]
    async fn debe_crear_link_de_pago_y_reutilizar_el_token() {
        let registro = Registro::default();
        let base = levantar(registro.clone()).await;
        let proveedor = Tilopay::nuevo(&config(&base, "crear")).expect("proveedor");

        let cargo = proveedor
            .crear_cargo("o-1", 12_345, "NIO")
            .await
            .expect("cargo");
        assert_eq!(proveedor.nombre(), "tilopay");
        assert_eq!(cargo.referencia, "o-1");
        assert_eq!(cargo.url.as_deref(), Some("https://pagos.example/link/1"));

        proveedor
            .crear_cargo("o-2", 100, "NIO")
            .await
            .expect("segundo cargo");
        assert_eq!(registro.login.load(Ordering::SeqCst), 1);

        let peticiones = registro.peticiones.lock().expect("peticiones");
        let (_, link) = peticiones
            .iter()
            .find(|(ruta, _)| ruta == "/api/v1/createLinkPayment")
            .expect("link");
        assert_eq!(link["amount"], "123.45");
        assert_eq!(link["currency"], "NIO");
        assert_eq!(link["reference"], "o-1");
        assert_eq!(link["callback_url"], "https://app.example/pago/retorno");
        assert_eq!(
            link["webhook_url"],
            "https://hooks.example/tilopay?order_id=o-1"
        );
    }

    #[tokio::test]
    async fn debe_traducir_reembolsos_y_consultas() {
        let registro = Registro::default();
        let base = levantar(registro.clone()).await;
        let proveedor = Tilopay::nuevo(&config(&base, "reembolso")).expect("proveedor");

        registro.reembolso_ok.store(true, Ordering::SeqCst);
        proveedor
            .reembolsar("o-1", 5_000)
            .await
            .expect("reembolso aprobado");
        registro.reembolso_ok.store(false, Ordering::SeqCst);
        let error = proveedor
            .reembolsar("o-1", 5_000)
            .await
            .expect_err("rechazo");
        assert_eq!(error.codigo, "REEMBOLSO_RECHAZADO");
        assert_eq!(error.mensaje, "Fondos insuficientes");

        *registro.consulta_codigo.lock().expect("consulta") = "1".to_string();
        assert_eq!(
            proveedor.consultar("o-1", None).await.expect("consulta"),
            EstadoExterno::Aprobado
        );
        *registro.consulta_codigo.lock().expect("consulta") = "2".to_string();
        assert_eq!(
            proveedor.consultar("o-1", None).await.expect("consulta"),
            EstadoExterno::Rechazado
        );

        let error = proveedor
            .contracargar("o-1", 100, "fraude")
            .await
            .expect_err("contracargo no soportado");
        assert_eq!(error.codigo, "PROVEEDOR_NO_SOPORTA_CONTRACARGO");
    }

    #[tokio::test]
    async fn debe_fallar_si_tilopay_no_esta_en_test() {
        let registro = Registro::default();
        let base = levantar(registro.clone()).await;
        let proveedor = Tilopay::nuevo(&config(&base, "ambiente")).expect("proveedor");

        *registro.consulta_codigo.lock().expect("consulta") = "1".to_string();
        *registro.ambiente.lock().expect("ambiente") = "Test".to_string();
        assert_eq!(
            proveedor.consultar("o-1", None).await.expect("consulta"),
            EstadoExterno::Aprobado
        );

        *registro.ambiente.lock().expect("ambiente") = "Production".to_string();
        let error = proveedor
            .consultar("o-1", None)
            .await
            .expect_err("debe rechazar produccion");
        assert_eq!(error.codigo, "TILOPAY_AMBIENTE_INESPERADO");
    }

    #[test]
    fn debe_exigir_credenciales_configuradas() {
        let error = match Tilopay::nuevo(&json!({"apiuser_env": "NO_EXISTE_TILOPAY_USER"})) {
            Ok(_) => panic!("no debe construirse sin credenciales"),
            Err(error) => error,
        };
        assert_eq!(error.codigo, "PROVEEDOR_PAGO_INCOMPLETO");
    }
}
