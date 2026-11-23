//! Métodos de pago configurables y registro de pagos por orden.
//!
//! El catálogo `orders.metodos_pago` es global y lo administra un admin. El
//! registro `orders.pagos` guarda intentos/confirmaciones; la confirmación
//! manual (efectivo/transferencia) mueve la orden a `pagada`.

use crate::db::{iso_ms, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::pasarelas::{pagadito::Pagadito, tilopay::Tilopay};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoExterno {
    pub referencia: String,
    /// URL de pago alojada cuando la pasarela la devuelve (Tilopay/Pagadito).
    pub url: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetodoPago {
    pub codigo: String,
    pub nombre: String,
    pub requiere_confirmacion: bool,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProveedorPago {
    pub codigo: String,
    pub nombre: String,
    pub tipo: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfiguracionPagos {
    pub empresa_id: String,
    pub negocio_id: Option<String>,
    pub proveedor_codigo: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pago {
    pub id: String,
    pub order_id: String,
    pub empresa_id: Option<String>,
    pub metodo_codigo: String,
    pub estado: String,
    pub monto_cents: i64,
    pub moneda: String,
    pub referencia: String,
    /// Identificador de la transacción en la pasarela (orderNumber de
    /// Tilopay, token de Pagadito). Vacío hasta que la pasarela lo informa.
    /// No se expone en la API: es un detalle operativo de la pasarela.
    #[serde(default, skip_serializing)]
    pub referencia_proveedor: String,
    pub proveedor: String,
    pub monto_reembolsado_cents: i64,
    pub motivo_rechazo: String,
    pub confirmado_por: Option<String>,
    pub conciliado_en: Option<String>,
    pub confirmado_en: Option<String>,
    pub creado_en: String,
}

pub struct Pagos {
    db: Arc<Db>,
    proveedor_default: Arc<dyn ProveedorPagos>,
}

impl Pagos {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self {
            db,
            proveedor_default: Arc::new(ProveedorSimulado),
        }
    }

    /// Proveedor global de fallback. Falla si la configuración declarada no
    /// puede construirse: arrancar simulando pagos cuando se pidió una pasarela
    /// real sería peor que no arrancar.
    pub fn con_config(db: Arc<Db>, config: &crate::config::Config) -> Result<Self, ApiError> {
        let tipo = config.pago_proveedor.trim().to_ascii_lowercase();
        let proveedor: Arc<dyn ProveedorPagos> = match tipo.as_str() {
            "http" => {
                let url = config.pago_proveedor_url.clone().ok_or_else(|| {
                    ApiError::dominio(
                        "PROVEEDOR_PAGO_INCOMPLETO",
                        "PAGO_PROVEEDOR=http requiere PAGO_PROVEEDOR_URL.",
                    )
                })?;
                Arc::new(ProveedorHttp::nuevo(url, config.pago_proveedor_key.clone()))
            }
            "tilopay" => {
                let mut configuracion = json!({});
                if let Some(url) = config.pago_proveedor_url.clone() {
                    configuracion["base_url"] = json!(url);
                }
                Arc::new(Tilopay::nuevo(&configuracion)?)
            }
            "pagadito" => {
                let mut configuracion = json!({});
                if let Some(url) = config.pago_proveedor_url.clone() {
                    configuracion["base_url"] = json!(url);
                }
                Arc::new(Pagadito::nuevo(&configuracion)?)
            }
            "manual" => Arc::new(ProveedorManual),
            "simulado" => Arc::new(ProveedorSimulado),
            otro => {
                return Err(ApiError::dominio(
                    "PROVEEDOR_PAGO_DESCONOCIDO",
                    format!("PAGO_PROVEEDOR={otro} no es un tipo de proveedor válido."),
                ))
            }
        };
        Ok(Self {
            db,
            proveedor_default: proveedor,
        })
    }

    pub async fn listar_proveedores(&self) -> Result<Vec<ProveedorPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_proveedor).collect()
    }

    pub async fn crear_proveedor(&self, datos: &Value) -> Result<ProveedorPago, ApiError> {
        let codigo = texto(datos, "codigo").to_lowercase();
        validar_codigo_proveedor(&codigo)?;
        let nombre = texto(datos, "nombre");
        if nombre.trim().is_empty() {
            return Err(ApiError::dominio(
                "PROVEEDOR_INVALIDO",
                "El nombre del proveedor es obligatorio.",
            ));
        }
        let tipo = texto(datos, "tipo").to_lowercase();
        validar_tipo_proveedor(&tipo)?;
        let activo = datos
            .get("activo")
            .and_then(Value::as_bool)
            .unwrap_or(tipo == "simulado" || tipo == "manual");
        let config = configuracion_de(datos);
        validar_config_proveedor(&tipo, activo, &config)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "INSERT INTO orders.proveedores_pago (codigo, nombre, tipo, activo, config)
             VALUES ($1, $2, $3, $4, $5::jsonb)
             ON CONFLICT (codigo) DO NOTHING
             RETURNING codigo, nombre, tipo, activo, config",
        )
        .bind(&codigo)
        .bind(nombre.trim())
        .bind(&tipo)
        .bind(activo)
        .bind(config.to_string())
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::conflicto("El proveedor de pago ya existe."))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_proveedor(&fila)
    }

    pub async fn actualizar_proveedor(
        &self,
        codigo: &str,
        datos: &Value,
    ) -> Result<ProveedorPago, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let actual = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago WHERE codigo = $1 FOR UPDATE",
        )
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(actual) = actual else {
            return Err(ApiError::no_encontrado("Proveedor de pago", codigo));
        };
        let actual = fila_proveedor(&actual)?;
        let tipo = texto(datos, "tipo").to_lowercase();
        let tipo = if tipo.is_empty() {
            actual.tipo.clone()
        } else {
            tipo
        };
        validar_tipo_proveedor(&tipo)?;
        let nombre = datos
            .get("nombre")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(&actual.nombre)
            .to_string();
        let activo = datos
            .get("activo")
            .and_then(Value::as_bool)
            .unwrap_or(actual.activo);
        let config = fusionar_config(&actual.config, &configuracion_de(datos));
        validar_config_proveedor(&tipo, activo, &config)?;
        let fila = sqlx::query(
            "UPDATE orders.proveedores_pago
                SET nombre = $2, tipo = $3, activo = $4, config = $5::jsonb,
                    actualizado_en = NOW()
              WHERE codigo = $1
              RETURNING codigo, nombre, tipo, activo, config",
        )
        .bind(codigo)
        .bind(nombre)
        .bind(tipo)
        .bind(activo)
        .bind(config.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_proveedor(&fila)
    }

    pub async fn configuracion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Option<ConfiguracionPagos>, ApiError> {
        if uuid::Uuid::parse_str(empresa_id).is_err() {
            return Ok(None);
        }
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT empresa_id, negocio_id, proveedor_codigo, activo, config
               FROM orders.configuracion_pagos
              WHERE empresa_id = $1::uuid
                AND ($2::uuid IS NULL OR negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC
              LIMIT 1",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_configuracion).transpose()
    }

    pub async fn guardar_configuracion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        datos: &Value,
    ) -> Result<ConfiguracionPagos, ApiError> {
        if uuid::Uuid::parse_str(empresa_id).is_err() {
            return Err(ApiError::dominio(
                "EMPRESA_INVALIDA",
                "El id de empresa no es un UUID valido.",
            ));
        }
        let proveedor_codigo = texto(datos, "proveedor_codigo").to_lowercase();
        if proveedor_codigo.is_empty() {
            return Err(ApiError::dominio(
                "PROVEEDOR_REQUERIDO",
                "Selecciona un proveedor de pago.",
            ));
        }
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = configuracion_de(datos);
        validar_config_tenant(&config)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let proveedor = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago WHERE codigo = $1",
        )
        .bind(&proveedor_codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(proveedor) = proveedor else {
            return Err(ApiError::no_encontrado(
                "Proveedor de pago",
                &proveedor_codigo,
            ));
        };
        let proveedor = fila_proveedor(&proveedor)?;
        if activo && !proveedor.activo {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INACTIVO",
                "El proveedor de pago no esta activo.",
            ));
        }
        validar_config_proveedor(
            &proveedor.tipo,
            activo,
            &fusionar_config_tenant(&proveedor.config, &config),
        )?;
        let fila = if negocio_id.is_some() {
            sqlx::query(
                "INSERT INTO orders.configuracion_pagos
                   (empresa_id, negocio_id, proveedor_codigo, activo, config)
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5::jsonb)
                 ON CONFLICT (empresa_id, negocio_id) WHERE negocio_id IS NOT NULL
                 DO UPDATE SET proveedor_codigo = EXCLUDED.proveedor_codigo,
                               activo = EXCLUDED.activo,
                               config = EXCLUDED.config,
                               actualizado_en = NOW()
                 RETURNING empresa_id, negocio_id, proveedor_codigo, activo, config",
            )
            .bind(empresa_id)
            .bind(negocio_id)
            .bind(&proveedor_codigo)
            .bind(activo)
            .bind(config.to_string())
            .fetch_one(&mut *tx)
            .await
        } else {
            sqlx::query(
                "INSERT INTO orders.configuracion_pagos
                   (empresa_id, negocio_id, proveedor_codigo, activo, config)
                 VALUES ($1::uuid, NULL, $2, $3, $4::jsonb)
                 ON CONFLICT (empresa_id) WHERE negocio_id IS NULL
                 DO UPDATE SET proveedor_codigo = EXCLUDED.proveedor_codigo,
                               activo = EXCLUDED.activo,
                               config = EXCLUDED.config,
                               actualizado_en = NOW()
                 RETURNING empresa_id, negocio_id, proveedor_codigo, activo, config",
            )
            .bind(empresa_id)
            .bind(&proveedor_codigo)
            .bind(activo)
            .bind(config.to_string())
            .fetch_one(&mut *tx)
            .await
        }
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_configuracion(&fila)
    }

    async fn proveedor_para(
        &self,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
    ) -> Result<(String, Arc<dyn ProveedorPagos>), ApiError> {
        let Some(empresa_id) = empresa_id else {
            return Ok((
                self.proveedor_default.nombre(),
                Arc::clone(&self.proveedor_default),
            ));
        };
        let Some(config) = self.configuracion(empresa_id, negocio_id).await? else {
            return Ok((
                self.proveedor_default.nombre(),
                Arc::clone(&self.proveedor_default),
            ));
        };
        if !config.activo {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INACTIVO",
                "La configuracion de pagos de la empresa esta inactiva.",
            ));
        }
        let proveedor = self
            .proveedor_por_codigo(empresa_id, negocio_id, &config.proveedor_codigo)
            .await?;
        Ok((config.proveedor_codigo, proveedor))
    }

    async fn proveedor_por_codigo(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        codigo: &str,
    ) -> Result<Arc<dyn ProveedorPagos>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT codigo, nombre, tipo, activo, config
               FROM orders.proveedores_pago WHERE codigo = $1",
        )
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Proveedor de pago", codigo));
        };
        let proveedor = fila_proveedor(&fila)?;
        if !proveedor.activo {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INACTIVO",
                "El proveedor de pago no esta activo.",
            ));
        }
        if proveedor.tipo == "simulado" {
            return Ok(Arc::new(ProveedorSimulado));
        }
        if proveedor.tipo == "manual" {
            return Ok(Arc::new(ProveedorManual));
        }
        let tenant_config = self
            .configuracion(empresa_id, negocio_id)
            .await?
            .filter(|config| config.proveedor_codigo == codigo)
            .map(|config| config.config)
            .unwrap_or_else(|| json!({}));
        let config = fusionar_config_tenant(&proveedor.config, &tenant_config);
        if proveedor.tipo == "tilopay" {
            return Ok(Arc::new(Tilopay::nuevo(&config)?));
        }
        if proveedor.tipo == "pagadito" {
            return Ok(Arc::new(Pagadito::nuevo(&config)?));
        }
        let Some(url) = config
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
        else {
            if codigo == self.proveedor_default.nombre() {
                return Ok(Arc::clone(&self.proveedor_default));
            }
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INCOMPLETO",
                "El proveedor HTTP requiere config.url.",
            ));
        };
        let clave = config
            .get("clave_env")
            .and_then(Value::as_str)
            .and_then(|variable| std::env::var(variable).ok());
        Ok(Arc::new(ProveedorHttp::nuevo(url.to_string(), clave)))
    }

    /// Secreto de webhook configurado por el proveedor/tenant. `None` deja que
    /// el handler use el secreto global como compatibilidad.
    pub async fn secreto_webhook(&self, pago_id: &str) -> Result<Option<String>, ApiError> {
        let Some(pago) = self.obtener(pago_id).await? else {
            return Ok(None);
        };
        let Some(empresa_id) = pago.empresa_id.as_deref() else {
            return Ok(None);
        };
        let Some(config) = self.configuracion(empresa_id, None).await? else {
            return Ok(None);
        };
        if config.proveedor_codigo != pago.proveedor {
            return Ok(None);
        }
        let mut base = config.config.clone();
        if let Ok(proveedores) = self.listar_proveedores().await {
            if let Some(proveedor) = proveedores.into_iter().find(|p| p.codigo == pago.proveedor) {
                base = fusionar_config_tenant(&proveedor.config, &base);
            }
        }
        Ok(base
            .get("webhook_secreto_env")
            .and_then(Value::as_str)
            .and_then(|variable| std::env::var(variable).ok())
            .filter(|valor| !valor.is_empty()))
    }

    async fn proveedor_de_pago(&self, pago: &Pago) -> Result<Arc<dyn ProveedorPagos>, ApiError> {
        let Some(empresa_id) = pago.empresa_id.as_deref() else {
            return Ok(Arc::clone(&self.proveedor_default));
        };
        match self
            .proveedor_por_codigo(empresa_id, None, &pago.proveedor)
            .await
        {
            Ok(proveedor) => Ok(proveedor),
            Err(_) if pago.proveedor == self.proveedor_default.nombre() => {
                Ok(Arc::clone(&self.proveedor_default))
            }
            Err(error) => Err(error),
        }
    }

    /// Resuelve un webhook sin confiar en su contenido: busca el pago por la
    /// referencia y consulta el estado real a la pasarela. Devuelve el pago y
    /// la decisión; el llamador mueve la orden con la saga existente.
    pub async fn resolver_webhook(
        &self,
        proveedor: &str,
        referencia: &str,
        datos: Option<&Value>,
    ) -> Result<(Pago, EstadoExterno), ApiError> {
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                    motivo_rechazo, confirmado_por, conciliado_en, confirmado_en, creado_en
               FROM orders.pagos
              WHERE referencia = $1 AND proveedor = $2
              ORDER BY creado_en DESC LIMIT 1",
        )
        .bind(referencia)
        .bind(proveedor)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Pago", referencia));
        };
        let pago = fila_pago(&fila)?;
        let pago = self
            .guardar_referencia_proveedor(pago, proveedor, datos)
            .await?;
        if matches!(
            pago.estado.as_str(),
            "confirmado" | "reembolsado" | "reembolsado_parcial"
        ) {
            return Ok((pago, EstadoExterno::Aprobado));
        }
        let proveedor = self.proveedor_de_pago(&pago).await?;
        let estado = proveedor
            .consultar(referencia_operativa(&pago), datos)
            .await?;
        Ok((pago, estado))
    }

    /// Persiste el identificador de la transacción que informa la pasarela.
    /// Tilopay lo necesita para consultar y reembolsar; Pagadito usa el token
    /// de la transacción para consultar el estado.
    async fn guardar_referencia_proveedor(
        &self,
        mut pago: Pago,
        proveedor: &str,
        datos: Option<&Value>,
    ) -> Result<Pago, ApiError> {
        if !pago.referencia_proveedor.is_empty() {
            return Ok(pago);
        }
        let Some(valor) = referencia_proveedor_de(proveedor, datos) else {
            return Ok(pago);
        };
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "UPDATE orders.pagos
                SET referencia_proveedor = $2, actualizado_en = NOW()
              WHERE id = $1::uuid AND referencia_proveedor = ''",
        )
        .bind(&pago.id)
        .bind(&valor)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        pago.referencia_proveedor = valor;
        Ok(pago)
    }

    /// Consulta el estado real de un pago ya cargado. Lo usa el job de
    /// conciliación para no volver a buscar la fila por referencia.
    pub async fn consultar_pago(
        &self,
        pago: &Pago,
        datos: Option<&Value>,
    ) -> Result<EstadoExterno, ApiError> {
        let proveedor = self.proveedor_de_pago(pago).await?;
        proveedor.consultar(referencia_operativa(pago), datos).await
    }

    /// Pagos pendientes con proveedor externo que ya superaron la ventana de
    /// gracia. El job los consulta y decide; nunca confirma por el webhook.
    /// Solo mira los últimos 30 días para acotar links abandonados.
    pub async fn pendientes_de_conciliacion(
        &self,
        minutos_gracia: u64,
        limite: i64,
    ) -> Result<Vec<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT p.id, p.order_id, p.empresa_id, p.metodo_codigo, p.estado, p.monto_cents,
                    p.moneda, p.referencia, p.referencia_proveedor, p.proveedor,
                    p.monto_reembolsado_cents, p.motivo_rechazo, p.confirmado_por,
                    p.conciliado_en, p.confirmado_en, p.creado_en
               FROM orders.pagos p
              WHERE p.estado = 'pendiente'
                AND p.referencia <> ''
                AND p.proveedor NOT IN ('manual')
                AND (
                      EXISTS (
                            SELECT 1
                              FROM orders.proveedores_pago pr
                             WHERE pr.codigo = p.proveedor
                               AND pr.tipo IN ('tilopay', 'pagadito')
                      )
                      OR (
                            p.proveedor = $3
                            AND NOT EXISTS (
                                  SELECT 1
                                    FROM orders.proveedores_pago pr2
                                   WHERE pr2.codigo = p.proveedor
                            )
                      )
                )
                AND p.creado_en < NOW() - ($1::int * INTERVAL '1 minute')
                AND p.creado_en > NOW() - INTERVAL '30 days'
              ORDER BY p.creado_en
              LIMIT $2",
        )
        .bind(minutos_gracia as i32)
        .bind(limite)
        .bind(self.proveedor_default.nombre())
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_pago).collect()
    }

    pub async fn listar_metodos(&self) -> Result<Vec<MetodoPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, requiere_confirmacion, activo, config
               FROM orders.metodos_pago ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_metodo).collect()
    }

    pub async fn listar_por_orden(&self, order_id: &str) -> Result<Vec<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                    motivo_rechazo, confirmado_por, conciliado_en,
                    confirmado_en, creado_en
               FROM orders.pagos
              WHERE order_id = $1::uuid
              ORDER BY creado_en DESC",
        )
        .bind(order_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_pago).collect()
    }

    pub async fn obtener(&self, pago_id: &str) -> Result<Option<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                    motivo_rechazo, confirmado_por, conciliado_en,
                    confirmado_en, creado_en
               FROM orders.pagos WHERE id = $1::uuid",
        )
        .bind(pago_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_pago).transpose()
    }

    pub async fn confirmar(&self, pago_id: &str, actor_id: &str) -> Result<Pago, ApiError> {
        self.procesar(pago_id, "confirmado", "", actor_id).await
    }

    pub async fn rechazar(
        &self,
        pago_id: &str,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        self.procesar(pago_id, "rechazado", motivo, actor_id).await
    }

    async fn procesar(
        &self,
        pago_id: &str,
        estado: &str,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.pagos
                SET estado = $2,
                    motivo_rechazo = CASE WHEN $2 = 'rechazado' THEN $3 ELSE motivo_rechazo END,
                    confirmado_por = $4::uuid,
                    confirmado_en = NOW(),
                    actualizado_en = NOW()
              WHERE id = $1::uuid AND estado = 'pendiente'
              RETURNING id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                        moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                        motivo_rechazo, confirmado_por, conciliado_en,
                        confirmado_en, creado_en",
        )
        .bind(pago_id)
        .bind(estado)
        .bind(motivo)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            let existe: Option<String> =
                sqlx::query_scalar("SELECT estado FROM orders.pagos WHERE id = $1::uuid")
                    .bind(pago_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(crate::event_store::error_db)?;
            return match existe {
                Some(estado) => Err(ApiError::conflicto(format!(
                    "El pago ya fue procesado (estado {estado})."
                ))),
                None => Err(ApiError::no_encontrado("Pago", pago_id)),
            };
        };
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_pago(&fila)
    }

    /// Deja constancia de un pago aprobado que no pudo aplicarse (por ejemplo,
    /// capturado después de que la orden expirara). No cambia su estado; un
    /// operador decide el reembolso con el flujo existente.
    pub async fn registrar_incidencia(
        &self,
        pago: &Pago,
        tipo: &str,
        motivo: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        self.registrar_evento(
            &mut tx,
            &pago.id,
            tipo,
            pago.monto_cents,
            motivo,
            None,
            json!({ "estado": pago.estado }),
        )
        .await?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(())
    }

    /// Método activo por código; `None` si no existe o está inactivo.
    pub async fn metodo_activo(&self, codigo: &str) -> Result<Option<MetodoPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT codigo, nombre, requiere_confirmacion, activo, config
               FROM orders.metodos_pago
              WHERE codigo = $1 AND activo = true",
        )
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_metodo).transpose()
    }

    /// Registra el pago de una orden. Los métodos automáticos nacen
    /// confirmados; los manuales quedan pendientes hasta la confirmación. Si
    /// el método declara `cargo_externo` (o es `simulado`), se crea el cargo en
    /// la pasarela antes de persistir la referencia.
    pub async fn registrar(
        &self,
        order_id: &str,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
        metodo: &MetodoPago,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<Option<String>, ApiError> {
        let cargo_externo = metodo
            .config
            .get("cargo_externo")
            .and_then(Value::as_bool)
            .unwrap_or(metodo.codigo == "simulado");
        let (proveedor_codigo, proveedor) = self.proveedor_para(empresa_id, negocio_id).await?;
        let simulado_permitido = metodo.codigo == "simulado"
            || metodo
                .config
                .get("simulado")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        if cargo_externo && proveedor.nombre() == "simulado" && !simulado_permitido {
            return Err(ApiError::dominio(
                "PAGO_SIMULADO_NO_PERMITIDO",
                "El método de pago requiere una pasarela real configurada.",
            ));
        }
        // Una pasarela real nunca nace confirmada: el dinero todavía no se ha
        // cobrado. Solo el simulado puede dar por pagada la orden al instante.
        let estado = if metodo.requiere_confirmacion
            || (cargo_externo && proveedor.nombre() != "simulado")
        {
            "pendiente"
        } else {
            "confirmado"
        };
        let cargo = if cargo_externo {
            Some(proveedor.crear_cargo(order_id, monto_cents, moneda).await?)
        } else {
            None
        };
        let proveedor = if cargo.is_some() {
            proveedor_codigo
        } else {
            "manual".to_string()
        };
        let referencia = cargo
            .as_ref()
            .map(|cargo| cargo.referencia.clone())
            .unwrap_or_default();
        let pago_url = cargo.and_then(|cargo| cargo.url);
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "INSERT INTO orders.pagos
               (order_id, empresa_id, metodo_codigo, estado, monto_cents, moneda,
                referencia, proveedor, confirmado_en)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8,
                     CASE WHEN $4 = 'confirmado' THEN NOW() ELSE NULL END)",
        )
        .bind(order_id)
        .bind(empresa_id)
        .bind(&metodo.codigo)
        .bind(estado)
        .bind(monto_cents)
        .bind(moneda)
        .bind(&referencia)
        .bind(&proveedor)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(pago_url)
    }

    /// Reembolso total o parcial. Actualiza el acumulado y deja auditoría.
    pub async fn reembolsar(
        &self,
        pago_id: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let pago = self
            .obtener(pago_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        if !matches!(pago.estado.as_str(), "confirmado" | "reembolsado_parcial") {
            return Err(ApiError::conflicto(format!(
                "El pago no admite reembolso en estado {}.",
                pago.estado
            )));
        }
        if monto_cents <= 0 || pago.monto_reembolsado_cents + monto_cents > pago.monto_cents {
            return Err(ApiError::dominio(
                "MONTO_REEMBOLSO_INVALIDO",
                "El reembolso debe ser positivo y no superar el monto del pago.",
            ));
        }
        if !pago.referencia.is_empty() {
            self.proveedor_de_pago(&pago)
                .await?
                .reembolsar(referencia_operativa(&pago), monto_cents)
                .await?;
        }
        let total_reembolsado = pago.monto_reembolsado_cents + monto_cents;
        let estado = if total_reembolsado == pago.monto_cents {
            "reembolsado"
        } else {
            "reembolsado_parcial"
        };
        let actualizado = self
            .actualizar_estado(
                pago_id,
                estado,
                total_reembolsado,
                "pago.reembolsado",
                monto_cents,
                motivo,
                actor_id,
            )
            .await?;
        Ok(actualizado)
    }

    /// Contracargo (chargeback) del emisor. No mueve la orden por sí solo.
    pub async fn contracargar(
        &self,
        pago_id: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let pago = self
            .obtener(pago_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        if pago.estado == "contracargo" {
            return Err(ApiError::conflicto("El pago ya tiene un contracargo."));
        }
        if monto_cents <= 0 || monto_cents > pago.monto_cents {
            return Err(ApiError::dominio(
                "MONTO_CONTRACARGO_INVALIDO",
                "El contracargo debe ser positivo y no superar el pago.",
            ));
        }
        if !pago.referencia.is_empty() {
            self.proveedor_de_pago(&pago)
                .await?
                .contracargar(referencia_operativa(&pago), monto_cents, motivo)
                .await?;
        }
        let total_reembolsado = (pago.monto_reembolsado_cents + monto_cents).min(pago.monto_cents);
        self.actualizar_estado(
            pago_id,
            "contracargo",
            total_reembolsado,
            "pago.contracargo",
            monto_cents,
            motivo,
            actor_id,
        )
        .await
    }

    /// Marca el pago como conciliado por un operador.
    pub async fn conciliar(&self, pago_id: &str, actor_id: &str) -> Result<Pago, ApiError> {
        let pago = self
            .obtener(pago_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.pagos
                SET conciliado_en = COALESCE(conciliado_en, NOW()), actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                        moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                        motivo_rechazo, confirmado_por, conciliado_en,
                        confirmado_en, creado_en",
        )
        .bind(pago_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        self.registrar_evento(
            &mut tx,
            pago_id,
            "pago.conciliado",
            0,
            "",
            Some(actor_id),
            json!({ "estado": pago.estado }),
        )
        .await?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_pago(&fila)
    }

    #[allow(clippy::too_many_arguments)]
    async fn actualizar_estado(
        &self,
        pago_id: &str,
        estado: &str,
        monto_reembolsado_cents: i64,
        evento: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.pagos
                SET estado = $2, monto_reembolsado_cents = $3, actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                        moneda, referencia, referencia_proveedor, proveedor, monto_reembolsado_cents,
                        motivo_rechazo, confirmado_por, conciliado_en,
                        confirmado_en, creado_en",
        )
        .bind(pago_id)
        .bind(estado)
        .bind(monto_reembolsado_cents)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Pago", pago_id))?;
        self.registrar_evento(
            &mut tx,
            pago_id,
            evento,
            monto_cents,
            motivo,
            Some(actor_id),
            json!({ "estado": estado }),
        )
        .await?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_pago(&fila)
    }

    #[allow(clippy::too_many_arguments)]
    async fn registrar_evento(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        pago_id: &str,
        tipo: &str,
        monto_cents: i64,
        motivo: &str,
        actor_id: Option<&str>,
        payload: Value,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO orders.pagos_eventos
               (pago_id, tipo, monto_cents, motivo, actor_id, payload)
             VALUES ($1::uuid, $2, $3, $4, $5::uuid, $6::jsonb)",
        )
        .bind(pago_id)
        .bind(tipo)
        .bind(monto_cents as i32)
        .bind(motivo)
        .bind(actor_id)
        .bind(payload)
        .execute(&mut **tx)
        .await
        .map_err(crate::event_store::error_db)?;
        Ok(())
    }

    pub async fn crear_metodo(&self, datos: &Value) -> Result<MetodoPago, ApiError> {
        let codigo = validar_codigo(campo(datos, "codigo"))?;
        let nombre = validar_nombre(campo(datos, "nombre"))?;
        let requiere_confirmacion = datos
            .get("requiere_confirmacion")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = config_de(datos)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "INSERT INTO orders.metodos_pago
               (codigo, nombre, requiere_confirmacion, activo, config)
             VALUES ($1, $2, $3, $4, $5::jsonb)
             RETURNING codigo, nombre, requiere_confirmacion, activo, config",
        )
        .bind(&codigo)
        .bind(&nombre)
        .bind(requiere_confirmacion)
        .bind(activo)
        .bind(&config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el metodo de pago."))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_metodo(&fila)
    }

    pub async fn actualizar_metodo(
        &self,
        codigo: &str,
        datos: &Value,
    ) -> Result<MetodoPago, ApiError> {
        let codigo = validar_codigo(codigo)?;
        let nombre = match datos.get("nombre") {
            Some(valor) => Some(validar_nombre(valor.as_str().unwrap_or_default())?),
            None => None,
        };
        let requiere_confirmacion = datos.get("requiere_confirmacion").and_then(Value::as_bool);
        let activo = datos.get("activo").and_then(Value::as_bool);
        let config = if datos.get("config").is_some() {
            Some(config_de(datos)?)
        } else {
            None
        };
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.metodos_pago
                SET nombre = COALESCE($2, nombre),
                    requiere_confirmacion = COALESCE($3, requiere_confirmacion),
                    activo = COALESCE($4, activo),
                    config = COALESCE($5::jsonb, config),
                    actualizado_en = NOW()
              WHERE codigo = $1
              RETURNING codigo, nombre, requiere_confirmacion, activo, config",
        )
        .bind(&codigo)
        .bind(nombre)
        .bind(requiere_confirmacion)
        .bind(activo)
        .bind(config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Metodo de pago", &codigo))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_metodo(&fila)
    }
}

/// Identificador de la transacción en la pasarela, si el aviso lo trae.
fn referencia_proveedor_de(proveedor: &str, datos: Option<&Value>) -> Option<String> {
    let datos = datos?;
    let valor = match proveedor {
        "tilopay" => datos.get("orderNumber").and_then(Value::as_str),
        "pagadito" => datos
            .get("resource")
            .and_then(|recurso| recurso.get("token"))
            .and_then(Value::as_str),
        _ => None,
    }?
    .trim();
    (!valor.is_empty()).then(|| valor.to_string())
}

/// Referencia que espera la pasarela para consultar o modificar el pago.
fn referencia_operativa(pago: &Pago) -> &str {
    if pago.referencia_proveedor.is_empty() {
        &pago.referencia
    } else {
        &pago.referencia_proveedor
    }
}

fn fila_pago(fila: &sqlx::postgres::PgRow) -> Result<Pago, ApiError> {
    let confirmado_en: Option<DateTime<Utc>> = fila
        .try_get("confirmado_en")
        .map_err(crate::event_store::error_db)?;
    let conciliado_en: Option<DateTime<Utc>> = fila
        .try_get("conciliado_en")
        .map_err(crate::event_store::error_db)?;
    let creado_en: DateTime<Utc> = fila
        .try_get("creado_en")
        .map_err(crate::event_store::error_db)?;
    Ok(Pago {
        id: uuid_texto(fila, "id").map_err(crate::event_store::error_db)?,
        order_id: uuid_texto(fila, "order_id").map_err(crate::event_store::error_db)?,
        empresa_id: uuid_opcional(fila, "empresa_id").map_err(crate::event_store::error_db)?,
        metodo_codigo: fila
            .try_get("metodo_codigo")
            .map_err(crate::event_store::error_db)?,
        estado: fila
            .try_get("estado")
            .map_err(crate::event_store::error_db)?,
        monto_cents: i64::from(
            fila.try_get::<i32, _>("monto_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        moneda: fila
            .try_get("moneda")
            .map_err(crate::event_store::error_db)?,
        referencia: fila
            .try_get("referencia")
            .map_err(crate::event_store::error_db)?,
        referencia_proveedor: fila
            .try_get("referencia_proveedor")
            .map_err(crate::event_store::error_db)?,
        proveedor: fila
            .try_get("proveedor")
            .map_err(crate::event_store::error_db)?,
        monto_reembolsado_cents: i64::from(
            fila.try_get::<i32, _>("monto_reembolsado_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        motivo_rechazo: fila
            .try_get("motivo_rechazo")
            .map_err(crate::event_store::error_db)?,
        confirmado_por: uuid_opcional(fila, "confirmado_por")
            .map_err(crate::event_store::error_db)?,
        conciliado_en: conciliado_en.map(iso_ms),
        confirmado_en: confirmado_en.map(iso_ms),
        creado_en: iso_ms(creado_en),
    })
}

fn fila_proveedor(fila: &sqlx::postgres::PgRow) -> Result<ProveedorPago, ApiError> {
    Ok(ProveedorPago {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        tipo: fila.try_get("tipo").map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}

fn fila_configuracion(fila: &sqlx::postgres::PgRow) -> Result<ConfiguracionPagos, ApiError> {
    Ok(ConfiguracionPagos {
        empresa_id: uuid_texto(fila, "empresa_id").map_err(crate::event_store::error_db)?,
        negocio_id: uuid_opcional(fila, "negocio_id").map_err(crate::event_store::error_db)?,
        proveedor_codigo: fila
            .try_get("proveedor_codigo")
            .map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}

fn texto(datos: &Value, clave: &str) -> String {
    campo(datos, clave).to_string()
}

fn fusionar_config(base: &Value, extra: &Value) -> Value {
    let mut salida = base.as_object().cloned().unwrap_or_default();
    if let Some(extra) = extra.as_object() {
        for (clave, valor) in extra {
            salida.insert(clave.clone(), valor.clone());
        }
    }
    Value::Object(salida)
}

/// Claves que solo puede definir la plataforma (admin), nunca el tenant:
/// permitir que el tenant las sobrescriba convierte la configuración de pagos
/// en un SSRF con exfiltración de variables de entorno.
const CONFIG_RESERVADA: [&str; 12] = [
    "url",
    "base_url",
    "clave_env",
    "apiuser_env",
    "password_env",
    "key_env",
    "uid_env",
    "wsk_env",
    "webhook_secreto_env",
    "webhook_url",
    "callback_url",
    "return_url",
];

/// Fusiona la config del proveedor con la del tenant, ignorando las claves
/// reservadas en la parte del tenant.
fn fusionar_config_tenant(base: &Value, extra: &Value) -> Value {
    let mut salida = base.as_object().cloned().unwrap_or_default();
    if let Some(extra) = extra.as_object() {
        for (clave, valor) in extra {
            if !CONFIG_RESERVADA.contains(&clave.as_str()) {
                salida.insert(clave.clone(), valor.clone());
            }
        }
    }
    Value::Object(salida)
}

fn validar_config_tenant(config: &Value) -> Result<(), ApiError> {
    if let Some(objeto) = config.as_object() {
        for clave in CONFIG_RESERVADA {
            if objeto.contains_key(clave) {
                return Err(ApiError::validacion(json!([
                    {"campo": format!("config.{clave}"), "restricciones": ["isReserved"]}
                ])));
            }
        }
    }
    Ok(())
}

fn configuracion_de(datos: &Value) -> Value {
    datos
        .get("config")
        .filter(|valor| valor.is_object())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn validar_tipo_proveedor(tipo: &str) -> Result<(), ApiError> {
    if ["simulado", "http", "manual", "tilopay", "pagadito"].contains(&tipo) {
        Ok(())
    } else {
        Err(ApiError::validacion(json!([
            {"campo": "tipo", "restricciones": ["isIn"]}
        ])))
    }
}

fn validar_codigo_proveedor(codigo: &str) -> Result<(), ApiError> {
    validar_codigo(codigo).map(|_| ())
}

fn validar_config_proveedor(tipo: &str, activo: bool, config: &Value) -> Result<(), ApiError> {
    if !config.is_object() {
        return Err(ApiError::validacion(json!([
            {"campo": "config", "restricciones": ["isObject"]}
        ])));
    }
    if tipo == "http" && activo {
        let url = config.get("url").and_then(Value::as_str).unwrap_or("");
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(ApiError::dominio(
                "PROVEEDOR_PAGO_INCOMPLETO",
                "Un proveedor HTTP activo requiere config.url http(s).",
            ));
        }
    }
    for clave in ["clave_env", "webhook_secreto_env"] {
        if let Some(valor) = config.get(clave) {
            if !valor.is_string() {
                return Err(ApiError::validacion(json!([
                    {"campo": clave, "restricciones": ["isString"]}
                ])));
            }
        }
    }
    Ok(())
}

fn fila_metodo(fila: &sqlx::postgres::PgRow) -> Result<MetodoPago, ApiError> {
    Ok(MetodoPago {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        requiere_confirmacion: fila
            .try_get("requiere_confirmacion")
            .map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}

fn campo<'a>(datos: &'a Value, clave: &str) -> &'a str {
    datos.get(clave).and_then(Value::as_str).unwrap_or("")
}

fn validar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "codigo", "restricciones": ["matches"]}
        ])));
    }
    Ok(codigo)
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim();
    if !(2..=120).contains(&nombre.chars().count()) {
        return Err(ApiError::validacion(json!([
            {"campo": "nombre", "restricciones": ["isString"]}
        ])));
    }
    Ok(nombre.to_string())
}

fn config_de(datos: &Value) -> Result<Value, ApiError> {
    let config = datos.get("config").cloned().unwrap_or_else(|| json!({}));
    if !config.is_object() {
        return Err(ApiError::validacion(json!([
            {"campo": "config", "restricciones": ["isObject"]}
        ])));
    }
    Ok(config)
}

fn conflicto_o_interno(error: sqlx::Error) -> ApiError {
    if let Some(codigo) = error.as_database_error().and_then(|db| db.code()) {
        if codigo == "23505" {
            return ApiError::conflicto("El metodo de pago ya existe.");
        }
    }
    crate::event_store::error_db(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_codigos_y_nombres() {
        assert_eq!(
            validar_codigo("Transferencia").expect("codigo"),
            "transferencia"
        );
        assert!(validar_codigo("1malo").is_err());
        assert!(validar_nombre("Efectivo").is_ok());
        assert!(validar_nombre("x").is_err());
    }

    #[test]
    fn debe_exigir_config_objeto() {
        assert_eq!(config_de(&json!({})).expect("config"), json!({}));
        assert!(config_de(&json!({"config": []})).is_err());
    }

    #[test]
    fn debe_rechazar_url_y_claves_de_entorno_del_tenant() {
        for clave in [
            "url",
            "clave_env",
            "webhook_secreto_env",
            "callback_url",
            "return_url",
        ] {
            let config = json!({ clave: "https://attacker.tld" });
            assert!(
                validar_config_tenant(&config).is_err(),
                "{clave} debe estar reservada"
            );
        }
        assert!(validar_config_tenant(&json!({"timeout_ms": 5000})).is_ok());
    }

    #[test]
    fn la_config_del_tenant_no_puede_sobrescribir_la_del_proveedor() {
        let base = json!({
            "url": "https://pasarela.example",
            "clave_env": "PAGO_PROVEEDOR_KEY",
            "timeout_ms": 5000
        });
        let tenant = json!({
            "url": "https://attacker.tld",
            "clave_env": "JWT_SECRET",
            "timeout_ms": 9000
        });
        let fusionada = fusionar_config_tenant(&base, &tenant);
        assert_eq!(fusionada["url"], "https://pasarela.example");
        assert_eq!(fusionada["clave_env"], "PAGO_PROVEEDOR_KEY");
        assert_eq!(fusionada["timeout_ms"], 9000);
    }

    #[test]
    fn debe_extraer_la_referencia_del_proveedor() {
        assert_eq!(
            referencia_proveedor_de("tilopay", Some(&json!({"orderNumber": " T-1 "}))),
            Some("T-1".to_string())
        );
        assert_eq!(
            referencia_proveedor_de("pagadito", Some(&json!({"resource": {"token": "tok"}}))),
            Some("tok".to_string())
        );
        assert_eq!(
            referencia_proveedor_de("pagadito", Some(&json!({"resource": {"ern": "o-1"}}))),
            None
        );
        assert_eq!(referencia_proveedor_de("tilopay", None), None);
    }

    #[tokio::test]
    async fn debe_crear_y_revertir_cargos_con_el_proveedor_simulado() {
        let proveedor = ProveedorSimulado;
        assert_eq!(proveedor.nombre(), "simulado");
        let cargo = proveedor
            .crear_cargo("orden-1", 10_000, "NIO")
            .await
            .expect("cargo");
        assert!(cargo.referencia.starts_with("sim-"));
        proveedor
            .reembolsar(&cargo.referencia, 5_000)
            .await
            .expect("reembolso");
        proveedor
            .contracargar(&cargo.referencia, 10_000, "fraude")
            .await
            .expect("contracargo");
    }
}
