//! Adaptador de Pagadito (APIPG) para la ACL `ProveedorPagos`.
//!
//! APIPG recibe formularios `application/x-www-form-urlencoded` en
//! `/apipg/charges.php`; cada operación lleva un `operation` con una clave
//! pública fija definida por Pagadito. Credenciales y URL se leen de
//! `orders.proveedores_pago.config` y de variables de entorno referenciadas por
//! nombre: no hay secretos en código.

use crate::errors::ApiError;
use crate::pagos::{CargoExterno, EstadoExterno, ProveedorPagos};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

const BASE_URL_DEFAULT: &str = "https://comercios.pagadito.com/apipg/charges.php";

// Claves públicas de operación publicadas por Pagadito (APIPG 1.5.x).
const OP_CONNECT: &str = "f3f191ce3326905ff4403bb05b0de150";
const OP_EXEC_TRANS: &str = "41216f8caf94aaa598db137e36d4673e";
const OP_GET_STATUS: &str = "0b50820c65b0de71ce78f6221a5cf876";

fn variable(config: &Value, clave: &str, default: &str) -> Result<String, ApiError> {
    let nombre = config
        .get(clave)
        .and_then(Value::as_str)
        .filter(|valor| !valor.is_empty())
        .unwrap_or(default);
    std::env::var(nombre).map_err(|_| {
        ApiError::dominio(
            "PROVEEDOR_PAGO_INCOMPLETO",
            format!("Falta la variable de entorno {nombre} para Pagadito."),
        )
    })
}

pub struct Pagadito {
    http: Client,
    base: String,
    uid: String,
    wsk: String,
    currency_default: String,
}

impl Pagadito {
    pub fn nuevo(config: &Value) -> Result<Self, ApiError> {
        let base = config
            .get("base_url")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(BASE_URL_DEFAULT)
            .to_string();
        Ok(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base,
            uid: variable(config, "uid_env", "PAGADITO_UID")?,
            wsk: variable(config, "wsk_env", "PAGADITO_WSK")?,
            currency_default: config
                .get("currency")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .unwrap_or("USD")
                .to_string(),
        })
    }

    async fn llamar(&self, parametros: &[(&str, String)]) -> Result<Value, ApiError> {
        let respuesta = self
            .http
            .post(&self.base)
            .form(parametros)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio("PROVEEDOR_PAGO_NO_DISPONIBLE", "Pagadito no responde.")
            })?;
        respuesta.json().await.map_err(|_| {
            ApiError::dominio("PROVEEDOR_PAGO_ERROR", "Respuesta inválida de Pagadito.")
        })
    }

    async fn conectar(&self) -> Result<String, ApiError> {
        let valor = self
            .llamar(&[
                ("operation", OP_CONNECT.to_string()),
                ("uid", self.uid.clone()),
                ("wsk", self.wsk.clone()),
                ("format_return", "json".to_string()),
            ])
            .await?;
        if valor.get("code").and_then(Value::as_str) != Some("PG1001") {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_ERROR",
                valor
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Pagadito rechazó la conexión.")
                    .to_string(),
            ));
        }
        valor
            .get("value")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ApiError::dominio("PROVEEDOR_PAGO_ERROR", "Pagadito no devolvió token."))
    }

    fn monto(monto_cents: i64) -> String {
        format!("{}.{:02}", monto_cents / 100, monto_cents.rem_euclid(100))
    }
}

#[async_trait]
impl ProveedorPagos for Pagadito {
    fn nombre(&self) -> String {
        "pagadito".to_string()
    }

    async fn crear_cargo(
        &self,
        order_id: &str,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        let token = self.conectar().await?;
        let moneda = if moneda.trim().is_empty() {
            self.currency_default.clone()
        } else {
            moneda.to_string()
        };
        let detalles = json!([{
            "quantity": 1,
            "description": format!("Orden {order_id}"),
            "price": monto_cents as f64 / 100.0,
            "url_product": ""
        }])
        .to_string();
        let valor = self
            .llamar(&[
                ("operation", OP_EXEC_TRANS.to_string()),
                ("token", token),
                ("ern", order_id.to_string()),
                ("amount", Self::monto(monto_cents)),
                ("details", detalles),
                ("format_return", "json".to_string()),
                ("currency", moneda),
            ])
            .await?;
        if valor.get("code").and_then(Value::as_str) != Some("PG1002") {
            return Err(ApiError::dominio(
                "PAGO_RECHAZADO",
                valor
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Pagadito rechazó el cargo.")
                    .to_string(),
            ));
        }
        // APIPG devuelve la URL codificada; el cliente oficial la decodifica.
        let url = valor
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ApiError::dominio(
                    "PROVEEDOR_PAGO_ERROR",
                    "Pagadito no devolvió la URL de pago.",
                )
            })
            .map(urlencoding::decode)
            .and_then(|valor| {
                valor.map(|url| url.into_owned()).map_err(|_| {
                    ApiError::dominio(
                        "PROVEEDOR_PAGO_ERROR",
                        "Pagadito devolvió una URL de pago inválida.",
                    )
                })
            })?;
        Ok(CargoExterno {
            referencia: order_id.to_string(),
            url: Some(url),
        })
    }

    async fn reembolsar(&self, _referencia: &str, _monto_cents: i64) -> Result<(), ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_NO_SOPORTA_REEMBOLSO",
            "Pagadito no documenta reembolsos por API; se gestionan desde su panel.",
        ))
    }

    async fn contracargar(
        &self,
        _referencia: &str,
        _monto_cents: i64,
        _motivo: &str,
    ) -> Result<(), ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_NO_SOPORTA_CONTRACARGO",
            "Pagadito no expone contracargos por API.",
        ))
    }

    async fn consultar(
        &self,
        referencia: &str,
        datos: Option<&Value>,
    ) -> Result<EstadoExterno, ApiError> {
        // El webhook trae `resource.token`; si no, la referencia operativa ya
        // es el token persistido en `orders.pagos.referencia_proveedor`.
        let token_trans = datos
            .and_then(|valor| {
                valor
                    .get("resource")
                    .and_then(|recurso| recurso.get("token"))
            })
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(referencia);
        let token = self.conectar().await?;
        let valor = self
            .llamar(&[
                ("operation", OP_GET_STATUS.to_string()),
                ("token", token),
                ("token_trans", token_trans.to_string()),
                ("format_return", "json".to_string()),
            ])
            .await?;
        if valor.get("code").and_then(Value::as_str) != Some("PG1003") {
            return Ok(EstadoExterno::Pendiente);
        }
        let estado = valor
            .get("value")
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("");
        match estado {
            "COMPLETED" => Ok(EstadoExterno::Aprobado),
            "FAILED" | "CANCELED" | "EXPIRED" | "REVOKED" => Ok(EstadoExterno::Rechazado),
            _ => Ok(EstadoExterno::Pendiente),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use axum::extract::{Form, State};
    use axum::routing::post;
    use axum::{Json, Router};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Registro {
        formularios: Arc<Mutex<Vec<HashMap<String, String>>>>,
        estado: Arc<Mutex<String>>,
        falla_conexion: Arc<AtomicBool>,
    }

    async fn responder(
        State(registro): State<Registro>,
        Form(campos): Form<HashMap<String, String>>,
    ) -> Json<Value> {
        registro
            .formularios
            .lock()
            .expect("registro")
            .push(campos.clone());
        let respuesta = match campos.get("operation").map(String::as_str) {
            Some(OP_CONNECT) => {
                if registro.falla_conexion.load(Ordering::SeqCst) {
                    json!({"code": "PG3001", "message": "Credenciales inválidas"})
                } else {
                    json!({"code": "PG1001", "value": "tok-sesion"})
                }
            }
            Some(OP_EXEC_TRANS) => json!({
                "code": "PG1002",
                "value": "https%3A%2F%2Fsandbox.pagadito.com%2Fpay%2F1"
            }),
            Some(OP_GET_STATUS) => json!({
                "code": "PG1003",
                "value": {"status": registro.estado.lock().expect("estado").clone()}
            }),
            _ => json!({"code": "PG9999", "message": "operación desconocida"}),
        };
        Json(respuesta)
    }

    async fn levantar(registro: Registro) -> String {
        let app = Router::new().fallback(post(responder)).with_state(registro);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("puerto");
        let direccion = format!(
            "http://{}/apipg/charges.php",
            listener.local_addr().expect("direccion")
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("servidor mock");
        });
        direccion
    }

    fn config(base: &str, sufijo: &str) -> Value {
        let uid = format!("PAGADITO_TEST_UID_{sufijo}");
        let wsk = format!("PAGADITO_TEST_WSK_{sufijo}");
        std::env::set_var(&uid, "uid");
        std::env::set_var(&wsk, "wsk");
        json!({
            "base_url": base,
            "uid_env": uid,
            "wsk_env": wsk,
            "currency": "USD",
        })
    }

    #[tokio::test]
    async fn debe_crear_la_transaccion_apipg() {
        let registro = Registro::default();
        let base = levantar(registro.clone()).await;
        let proveedor = Pagadito::nuevo(&config(&base, "crear")).expect("proveedor");

        let cargo = proveedor
            .crear_cargo("o-1", 12_345, "NIO")
            .await
            .expect("cargo");
        assert_eq!(proveedor.nombre(), "pagadito");
        assert_eq!(cargo.referencia, "o-1");
        assert_eq!(
            cargo.url.as_deref(),
            Some("https://sandbox.pagadito.com/pay/1")
        );

        let formularios = registro.formularios.lock().expect("formularios");
        let conexion = formularios
            .iter()
            .find(|campos| campos["operation"] == OP_CONNECT)
            .expect("connect");
        assert_eq!(conexion["uid"], "uid");
        assert_eq!(conexion["wsk"], "wsk");
        assert_eq!(conexion["format_return"], "json");

        let transaccion = formularios
            .iter()
            .find(|campos| campos["operation"] == OP_EXEC_TRANS)
            .expect("exec_trans");
        assert_eq!(transaccion["token"], "tok-sesion");
        assert_eq!(transaccion["ern"], "o-1");
        assert_eq!(transaccion["amount"], "123.45");
        assert_eq!(transaccion["currency"], "NIO");
        let detalles: Value = serde_json::from_str(&transaccion["details"]).expect("detalles JSON");
        assert_eq!(detalles[0]["price"], json!(123.45));
        assert!(transaccion["details"].contains("Orden o-1"));
    }

    #[tokio::test]
    async fn debe_traducir_los_estados_de_pagadito() {
        let registro = Registro::default();
        let base = levantar(registro.clone()).await;
        let proveedor = Pagadito::nuevo(&config(&base, "estado")).expect("proveedor");

        for (estado, esperado) in [
            ("COMPLETED", EstadoExterno::Aprobado),
            ("REGISTERED", EstadoExterno::Pendiente),
            ("VERIFYING", EstadoExterno::Pendiente),
            ("FAILED", EstadoExterno::Rechazado),
            ("CANCELED", EstadoExterno::Rechazado),
            ("EXPIRED", EstadoExterno::Rechazado),
            ("REVOKED", EstadoExterno::Rechazado),
        ] {
            *registro.estado.lock().expect("estado") = estado.to_string();
            assert_eq!(
                proveedor.consultar("o-1", None).await.expect("consulta"),
                esperado,
                "estado {estado}"
            );
        }

        let datos = json!({"resource": {"token": "token-transaccion"}});
        *registro.estado.lock().expect("estado") = "COMPLETED".to_string();
        proveedor
            .consultar("o-1", Some(&datos))
            .await
            .expect("consulta");
        let formularios = registro.formularios.lock().expect("formularios");
        let ultima = formularios
            .iter()
            .rev()
            .find(|campos| campos["operation"] == OP_GET_STATUS)
            .expect("get_status");
        assert_eq!(ultima["token_trans"], "token-transaccion");
    }

    #[tokio::test]
    async fn debe_rechazar_operaciones_no_soportadas() {
        let registro = Registro::default();
        let base = levantar(registro.clone()).await;
        let proveedor = Pagadito::nuevo(&config(&base, "errores")).expect("proveedor");

        let error = proveedor
            .reembolsar("o-1", 100)
            .await
            .expect_err("reembolso");
        assert_eq!(error.codigo, "PROVEEDOR_NO_SOPORTA_REEMBOLSO");
        let error = proveedor
            .contracargar("o-1", 100, "fraude")
            .await
            .expect_err("contracargo");
        assert_eq!(error.codigo, "PROVEEDOR_NO_SOPORTA_CONTRACARGO");

        registro.falla_conexion.store(true, Ordering::SeqCst);
        let error = proveedor
            .crear_cargo("o-1", 100, "USD")
            .await
            .expect_err("conexión rechazada");
        assert_eq!(error.codigo, "PROVEEDOR_PAGO_ERROR");
        assert_eq!(error.mensaje, "Credenciales inválidas");

        let error = match Pagadito::nuevo(&json!({"uid_env": "NO_EXISTE_PAGADITO_UID"})) {
            Ok(_) => panic!("no debe construirse sin credenciales"),
            Err(error) => error,
        };
        assert_eq!(error.codigo, "PROVEEDOR_PAGO_INCOMPLETO");
    }
}
