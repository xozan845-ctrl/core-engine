use super::prelude::*;
use async_trait::async_trait;
use serde_json::json;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CargoExterno {
    pub referencia: String,
    /// URL de pago alojada cuando la pasarela la devuelve (Tilopay/Pagadito).
    pub url: Option<String>,
    /// Datos de liquidación que devuelve la pasarela al crear el cargo.
    /// La mayoría de pasarelas los entrega en la consulta/webhook, no aquí.
    pub liquidacion: Option<LiquidacionPasarela>,
}

/// Comisión, impuestos, retenciones y neto que reporta la pasarela.
///
/// Todos los montos se guardan en centavos de la moneda de liquidación. Si la
/// pasarela los devuelve en unidades mayores (p. ej. `3.50`), el adaptador los
/// convierte según `config.liquidacion_unidad`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LiquidacionPasarela {
    pub proveedor: String,
    pub comision_cents: i64,
    pub impuesto_cents: i64,
    pub retencion_cents: i64,
    pub monto_liquidado_cents: i64,
    pub moneda: String,
    #[serde(default)]
    pub detalle: Value,
}

/// Resultado de consultar la pasarela: estado y, si está disponible, el
/// desglose de liquidación para el ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsultaExterna {
    pub estado: EstadoExterno,
    pub liquidacion: Option<LiquidacionPasarela>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoExterno {
    Aprobado,
    Rechazado,
    Pendiente,
}

/// ACL de la pasarela de pagos (R-AR-6). El dominio solo conoce esta interfaz;
/// los adaptadores concretos se seleccionan por configuración.
#[async_trait]
pub trait ProveedorPagos: Send + Sync {
    fn nombre(&self) -> String;
    async fn crear_cargo(
        &self,
        order_id: &str,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<CargoExterno, ApiError>;
    async fn reembolsar(&self, referencia: &str, monto_cents: i64) -> Result<(), ApiError>;
    async fn contracargar(
        &self,
        referencia: &str,
        monto_cents: i64,
        motivo: &str,
    ) -> Result<(), ApiError>;
    /// Consulta el estado real en la pasarela. El webhook solo es una señal;
    /// la decisión se toma con esta llamada.
    async fn consultar(
        &self,
        _referencia: &str,
        _datos: Option<&Value>,
    ) -> Result<EstadoExterno, ApiError> {
        Ok(EstadoExterno::Pendiente)
    }

    /// Igual que `consultar`, pero devuelve también el desglose de liquidación
    /// cuando la pasarela lo informa. Los adaptadores concretos lo sobrescriben
    /// para aprovechar la misma respuesta HTTP.
    async fn consultar_detallado(
        &self,
        referencia: &str,
        datos: Option<&Value>,
    ) -> Result<ConsultaExterna, ApiError> {
        Ok(ConsultaExterna {
            estado: self.consultar(referencia, datos).await?,
            liquidacion: None,
        })
    }
}

/// Extrae el desglose de liquidación de una respuesta de pasarela. Devuelve
/// `None` si el cuerpo no trae ninguno de los campos reconocidos, para no
/// sobrescribir datos previos con ceros.
pub(crate) fn liquidacion_de(
    proveedor: &str,
    valor: &Value,
    unidad: &str,
) -> Option<LiquidacionPasarela> {
    let fuente = valor
        .get("liquidacion")
        .or_else(|| valor.get("settlement"))
        .or_else(|| valor.get("detalle_liquidacion"))
        .unwrap_or(valor);
    if !fuente.is_object() {
        return None;
    }
    let comision = monto(
        fuente,
        &[
            "comision_cents",
            "commission_cents",
            "comision_pasarela_cents",
            "comision",
            "commission",
            "fee",
            "gateway_fee",
            "processing_fee",
        ],
        unidad,
    );
    let impuesto = monto(
        fuente,
        &[
            "impuesto_cents",
            "iva_cents",
            "tax_cents",
            "impuesto",
            "iva",
            "tax",
        ],
        unidad,
    );
    let retencion = monto(
        fuente,
        &[
            "retencion_cents",
            "withholding_cents",
            "retencion",
            "withholding",
        ],
        unidad,
    );
    let neto = monto(
        fuente,
        &[
            "monto_liquidado_cents",
            "neto_cents",
            "net_cents",
            "monto_liquidado",
            "neto",
            "net_amount",
            "amount_net",
        ],
        unidad,
    );
    let moneda = texto(
        fuente,
        &[
            "moneda_liquidacion",
            "settlement_currency",
            "currency",
            "moneda",
        ],
    )
    .map(|valor| valor.to_uppercase());
    if comision.is_none()
        && impuesto.is_none()
        && retencion.is_none()
        && neto.is_none()
        && moneda.is_none()
    {
        return None;
    }
    Some(LiquidacionPasarela {
        proveedor: proveedor.to_string(),
        comision_cents: comision.unwrap_or(0).max(0),
        impuesto_cents: impuesto.unwrap_or(0).max(0),
        retencion_cents: retencion.unwrap_or(0).max(0),
        monto_liquidado_cents: neto.unwrap_or(0).max(0),
        moneda: moneda.unwrap_or_default(),
        detalle: fuente.clone(),
    })
}

fn monto(valor: &Value, claves: &[&str], unidad: &str) -> Option<i64> {
    for clave in claves {
        let Some(valor) = valor.get(*clave) else {
            continue;
        };
        let numero = match valor {
            Value::Number(numero) => numero.as_f64()?,
            Value::String(texto) => texto.trim().parse::<f64>().ok()?,
            _ => continue,
        };
        let centavos = if clave.contains("cents") || unidad.eq_ignore_ascii_case("cents") {
            numero.round() as i64
        } else {
            (numero * 100.0).round() as i64
        };
        return Some(centavos);
    }
    None
}

fn texto(valor: &Value, claves: &[&str]) -> Option<String> {
    for clave in claves {
        if let Some(texto) = valor.get(*clave).and_then(Value::as_str) {
            let texto = texto.trim();
            if !texto.is_empty() {
                return Some(texto.to_string());
            }
        }
    }
    None
}

pub struct ProveedorSimulado;

#[async_trait]
impl ProveedorPagos for ProveedorSimulado {
    fn nombre(&self) -> String {
        "simulado".to_string()
    }

    async fn crear_cargo(
        &self,
        _order_id: &str,
        _monto_cents: i64,
        _moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        Ok(CargoExterno {
            referencia: format!("sim-{}", Uuid::new_v4()),
            url: None,
            ..CargoExterno::default()
        })
    }

    async fn reembolsar(&self, _referencia: &str, _monto_cents: i64) -> Result<(), ApiError> {
        Ok(())
    }

    async fn contracargar(
        &self,
        _referencia: &str,
        _monto_cents: i64,
        _motivo: &str,
    ) -> Result<(), ApiError> {
        Ok(())
    }
}

/// Proveedor manual: no crea cargos ni reembolsa por API. Se usa para
/// efectivo/transferencia o cuando el negocio concilia fuera del sistema.
pub struct ProveedorManual;

#[async_trait]
impl ProveedorPagos for ProveedorManual {
    fn nombre(&self) -> String {
        "manual".to_string()
    }

    async fn crear_cargo(
        &self,
        _order_id: &str,
        _monto_cents: i64,
        _moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_MANUAL",
            "El proveedor manual no crea cargos externos.",
        ))
    }

    async fn reembolsar(&self, _referencia: &str, _monto_cents: i64) -> Result<(), ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_MANUAL",
            "El proveedor manual no procesa reembolsos por API.",
        ))
    }

    async fn contracargar(
        &self,
        _referencia: &str,
        _monto_cents: i64,
        _motivo: &str,
    ) -> Result<(), ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_MANUAL",
            "El proveedor manual no procesa contracargos por API.",
        ))
    }
}

/// Adaptador HTTP genérico para una pasarela que exponga `/cargos`,
/// `/reembolsos` y `/contracargos` con `Authorization: Bearer`.
pub struct ProveedorHttp {
    http: Client,
    url: String,
    clave: String,
}

impl ProveedorHttp {
    pub fn nuevo(url: String, clave: Option<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            http,
            url: url.trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
        }
    }

    fn peticion(&self, ruta: &str) -> reqwest::RequestBuilder {
        self.http
            .post(format!("{}/{}", self.url, ruta))
            .bearer_auth(&self.clave)
    }
}

#[async_trait]
impl ProveedorPagos for ProveedorHttp {
    fn nombre(&self) -> String {
        "http".to_string()
    }

    async fn crear_cargo(
        &self,
        order_id: &str,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        let respuesta = self
            .peticion("cargos")
            .json(&json!({
                "orden_id": order_id,
                "monto_cents": monto_cents,
                "moneda": moneda,
            }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pasarela de pagos"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "PAGO_RECHAZADO",
                "La pasarela rechazo la creacion del cargo.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        let valor: Value = respuesta.json().await.map_err(|_| ApiError::interno())?;
        let referencia = valor
            .get("referencia")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .ok_or_else(ApiError::interno)?
            .to_string();
        Ok(CargoExterno {
            referencia,
            url: None,
            ..CargoExterno::default()
        })
    }

    async fn reembolsar(&self, referencia: &str, monto_cents: i64) -> Result<(), ApiError> {
        let respuesta = self
            .peticion("reembolsos")
            .json(&json!({ "referencia": referencia, "monto_cents": monto_cents }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pasarela de pagos"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "REEMBOLSO_RECHAZADO",
                "La pasarela rechazo el reembolso.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        Ok(())
    }

    async fn contracargar(
        &self,
        referencia: &str,
        monto_cents: i64,
        motivo: &str,
    ) -> Result<(), ApiError> {
        let respuesta = self
            .peticion("contracargos")
            .json(&json!({
                "referencia": referencia,
                "monto_cents": monto_cents,
                "motivo": motivo,
            }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pasarela de pagos"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "CONTRACARGO_RECHAZADO",
                "La pasarela rechazo el contracargo.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        Ok(())
    }
}
