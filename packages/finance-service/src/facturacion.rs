//! Comprobantes fiscales DGI (cap. 4.4): series con prefijo, secuenciales
//! consecutivos y RUC del emisor. Ley 842: facturación en córdobas (C$).
//! Ley 822: IVA del 15 % se traslada al comprador.

use crate::contabilidad::{Contabilidad, DetalleAsiento, ParametrosAsiento};
use crate::db::{jsonb, timestamp, uuid_texto, Db};
use crate::errors::{error_db, ApiError};
use crate::money;
use crate::outbox::{Outbox, COMPROBANTE_EMITIDO};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comprobante {
    pub id: String,
    pub serie: String,
    pub numero: String,
    pub tipo: String,
    pub orden_id: Option<String>,
    pub empresa_id: Option<String>,
    pub parte_id: Option<String>,
    pub negocio_id: Option<String>,
    pub base_gravada_cents: i64,
    pub iva_cents: i64,
    pub exento_cents: i64,
    pub total_cents: i64,
    pub moneda: String,
    pub estado: String,
    pub datos_cliente: Option<Value>,
    pub emitido_en: Option<String>,
    pub creado_en: String,
    pub electronica_estado: Option<String>,
    pub electronica_referencia: Option<String>,
    pub electronica_enviada_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerieComprobante {
    pub serie: String,
    pub negocio_id: Option<String>,
    pub tipo: String,
    pub prefijo: String,
    pub secuencial_actual: i64,
    pub ruc_emisor: String,
    pub jurisdiccion: String,
    pub activa: bool,
}

/// Consulta de la orden a orders-service (lectura interna).
#[async_trait]
pub trait Ordenes: Send + Sync {
    async fn consultar(&self, order_id: &str) -> Option<OrdenResumen>;
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OrdenItemFiscal {
    #[serde(default)]
    pub sku: String,
    #[serde(default)]
    pub precio_unitario_cents: i64,
    #[serde(default)]
    pub cantidad: i64,
    #[serde(default)]
    pub descuento_cents: i64,
    #[serde(default)]
    pub impuesto_cents: i64,
    #[serde(default)]
    pub total_linea_cents: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OrdenResumen {
    pub total_cents: i64,
    #[serde(default)]
    pub empresa_id: Option<String>,
    #[serde(default)]
    pub negocio_id: Option<String>,
    #[serde(default)]
    pub items: Vec<OrdenItemFiscal>,
    #[serde(default)]
    pub descuento_cupon_cents: i64,
}

pub struct OrdenesHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl OrdenesHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl Ordenes for OrdenesHttp {
    async fn consultar(&self, order_id: &str) -> Option<OrdenResumen> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .ok()?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/orders/{}",
                self.url,
                urlencoding::encode(order_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let valor: Value = respuesta.json().await.ok()?;
        let mut orden: OrdenResumen = serde_json::from_value(valor.clone()).ok()?;
        if orden.empresa_id.is_none() {
            orden.empresa_id = valor
                .get("items")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("empresa_id"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        Some(orden)
    }
}

pub struct SinOrdenes;

#[async_trait]
impl Ordenes for SinOrdenes {
    async fn consultar(&self, _order_id: &str) -> Option<OrdenResumen> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenciaElectronica {
    pub referencia: String,
    pub estado: String,
}

/// ACL para facturación electrónica. El dominio no conoce DGI ni el proveedor;
/// un adaptador HTTP o el simulado se eligen por configuración del tenant.
#[async_trait]
pub trait ProveedorFacturacionElectronica: Send + Sync {
    async fn emitir(
        &self,
        comprobante: &Comprobante,
        ambiente: &str,
    ) -> Result<ReferenciaElectronica, ApiError>;
}

pub struct FacturacionSimulada;

#[async_trait]
impl ProveedorFacturacionElectronica for FacturacionSimulada {
    async fn emitir(
        &self,
        _comprobante: &Comprobante,
        _ambiente: &str,
    ) -> Result<ReferenciaElectronica, ApiError> {
        Ok(ReferenciaElectronica {
            referencia: format!("sim-{}", uuid::Uuid::new_v4()),
            estado: "aceptada".to_string(),
        })
    }
}

pub struct FacturacionHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl FacturacionHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms.max(1)),
        }
    }
}

#[async_trait]
impl ProveedorFacturacionElectronica for FacturacionHttp {
    async fn emitir(
        &self,
        comprobante: &Comprobante,
        ambiente: &str,
    ) -> Result<ReferenciaElectronica, ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ApiError::interno())?;
        let respuesta = cliente
            .post(format!("{}/comprobantes", self.url))
            .bearer_auth(&self.clave)
            .json(&json!({ "comprobante": comprobante, "ambiente": ambiente }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("facturacion-electronica"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "FACTURACION_RECHAZADA",
                "El proveedor rechazo el comprobante.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        let valor: Value = respuesta.json().await.map_err(|_| ApiError::interno())?;
        Ok(ReferenciaElectronica {
            referencia: valor
                .get("referencia")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            estado: valor
                .get("estado")
                .and_then(Value::as_str)
                .unwrap_or("aceptada")
                .to_string(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EmitirComprobante {
    pub tipo: String,
    pub orden_id: String,
    #[serde(default)]
    pub parte_id: Option<String>,
    #[serde(default)]
    pub razon_social: Option<String>,
    #[serde(default)]
    pub ruc: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct NotaCreditoDto {
    pub orden_id: String,
    pub devolucion_id: String,
    pub empresa_id: String,
    #[serde(default)]
    pub negocio_id: Option<String>,
    #[serde(default)]
    pub parte_id: Option<String>,
    pub base_cents: i64,
    pub iva_cents: i64,
    pub total_cents: i64,
    #[serde(default)]
    pub moneda: Option<String>,
    #[serde(default)]
    pub items: Value,
}

pub struct Facturacion {
    db: Arc<Db>,
    outbox: Outbox,
    contabilidad: Arc<Contabilidad>,
    ordenes: Arc<dyn Ordenes>,
    emitir: bool,
    http: Option<Arc<FacturacionHttp>>,
    pdf_almacen: Option<Arc<crate::pdf_almacen::PdfAlmacen>>,
}

impl Facturacion {
    pub fn nueva(
        db: Arc<Db>,
        outbox: Outbox,
        contabilidad: Arc<Contabilidad>,
        ordenes: Arc<dyn Ordenes>,
        emitir: bool,
    ) -> Self {
        Self {
            db,
            outbox,
            contabilidad,
            ordenes,
            emitir,
            http: None,
            pdf_almacen: None,
        }
    }

    pub fn con_pdf_almacen(mut self, almacen: crate::pdf_almacen::PdfAlmacen) -> Self {
        self.pdf_almacen = Some(Arc::new(almacen));
        self
    }

    pub fn con_proveedor_http(
        mut self,
        url: String,
        clave: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        self.http = Some(Arc::new(FacturacionHttp::nueva(url, clave, timeout_ms)));
        self
    }

    pub async fn listar(
        &self,
        desde: Option<DateTime<Utc>>,
        hasta: Option<DateTime<Utc>>,
        estado: Option<&str>,
        negocio_id: Option<&str>,
    ) -> Result<Vec<Comprobante>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id, base_gravada_cents,
                    iva_cents, exento_cents, total_cents, moneda, estado,
                    datos_cliente, emitido_en, creado_en,
                    electronica_estado, electronica_referencia, electronica_enviada_en
             FROM finance.comprobantes
             WHERE ($1::timestamptz IS NULL OR creado_en >= $1)
               AND ($2::timestamptz IS NULL OR creado_en <= $2)
               AND ($3::text IS NULL OR estado = $3)
               AND ($4::uuid IS NULL OR negocio_id = $4::uuid)
             ORDER BY creado_en DESC
             LIMIT 300",
        )
        .bind(desde)
        .bind(hasta)
        .bind(estado)
        .bind(negocio_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(comprobante_de_fila).collect())
    }

    pub async fn series(&self) -> Result<Vec<SerieComprobante>, ApiError> {
        self.series_de(None).await
    }

    pub async fn series_de(
        &self,
        negocio_id: Option<&str>,
    ) -> Result<Vec<SerieComprobante>, ApiError> {
        let filas = sqlx::query(
            "SELECT serie, negocio_id, tipo, prefijo, secuencial_actual, ruc_emisor,
                    jurisdiccion, activa
             FROM finance.series_comprobantes
              WHERE ($1::uuid IS NULL OR negocio_id IS NULL OR negocio_id = $1::uuid)
              ORDER BY serie",
        )
        .bind(negocio_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(serie_de_fila).collect())
    }

    /// Emite (o reserva como BORRADOR) un comprobante de la venta.
    pub async fn emitir(&self, dto: EmitirComprobante) -> Result<Comprobante, ApiError> {
        if dto.tipo != "FACTURA" && dto.tipo != "NOTA_CREDITO" {
            return Err(ApiError::validacion(json!([
                {"campo": "tipo", "restricciones": ["isIn"]}
            ])));
        }
        let orden = self
            .ordenes
            .consultar(&dto.orden_id)
            .await
            .ok_or_else(|| ApiError::no_encontrado("Orden", &dto.orden_id))?;
        let negocio_uuid = orden
            .negocio_id
            .as_deref()
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ApiError::interno())?;

        let serie = sqlx::query(
            "SELECT serie, negocio_id, tipo, prefijo, secuencial_actual, ruc_emisor,
                    jurisdiccion, activa
             FROM finance.series_comprobantes
              WHERE tipo = $1 AND activa
                AND (negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC, serie
              LIMIT 1",
        )
        .bind(&dto.tipo)
        .bind(negocio_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "SERIE_NO_DISPONIBLE",
                format!("No hay serie activa para {}.", dto.tipo),
            )
        })?;
        let serie = serie_de_fila(&serie);

        let fila_jurisdiccion = sqlx::query(
            "SELECT tasa_iva, moneda FROM finance.jurisdicciones WHERE codigo_pais = $1",
        )
        .bind(&serie.jurisdiccion)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let (tasa_iva, moneda) = match fila_jurisdiccion {
            Some(fila) => (
                fila.try_get::<f64, _>("tasa_iva").unwrap_or(0.15),
                fila.try_get::<String, _>("moneda")
                    .unwrap_or_else(|_| "NIO".to_string()),
            ),
            None => (0.15, "NIO".to_string()),
        };
        let (base, iva) = if orden.items.is_empty() {
            let base = money::desde_centavos(orden.total_cents)?;
            let iva = money::multiplicar_por(base, tasa_iva)?;
            (base, iva)
        } else {
            let mut base = 0;
            let mut iva = 0;
            let con_snapshot = orden
                .items
                .iter()
                .any(|item| item.total_linea_cents > 0 || item.impuesto_cents > 0);
            for item in &orden.items {
                let neto = item
                    .precio_unitario_cents
                    .saturating_mul(item.cantidad)
                    .saturating_sub(item.descuento_cents);
                if neto <= 0 && item.total_linea_cents <= 0 {
                    continue;
                }
                if item.total_linea_cents > 0 || item.impuesto_cents > 0 {
                    let base_linea = item
                        .total_linea_cents
                        .saturating_sub(item.impuesto_cents)
                        .max(0);
                    base = money::sumar(base, money::desde_centavos(base_linea)?)?;
                    iva = money::sumar(iva, money::desde_centavos(item.impuesto_cents)?)?;
                } else {
                    let base_linea = money::desde_centavos(neto)?;
                    base = money::sumar(base, base_linea)?;
                    iva = money::sumar(iva, money::multiplicar_por(base_linea, tasa_iva)?)?;
                }
            }
            if !con_snapshot && orden.descuento_cupon_cents > 0 && base > 0 {
                let cupon = orden.descuento_cupon_cents.min(base);
                base -= cupon;
                iva = money::multiplicar_por(base, tasa_iva)?;
            }
            (base, iva)
        };
        let total = money::sumar(base, iva)?;
        let numero = format!("{}{:08}", serie.prefijo, serie.secuencial_actual + 1);
        let estado = if self.emitir { "EMITIDO" } else { "BORRADOR" };
        let parte_id = dto.parte_id.clone();
        let items_fiscales: Vec<Value> = orden
            .items
            .iter()
            .map(|item| {
                let neto = item
                    .precio_unitario_cents
                    .saturating_mul(item.cantidad)
                    .saturating_sub(item.descuento_cents);
                let (base_linea, iva_linea, total_linea) =
                    if item.total_linea_cents > 0 || item.impuesto_cents > 0 {
                        let base = item
                            .total_linea_cents
                            .saturating_sub(item.impuesto_cents)
                            .max(0);
                        (base, item.impuesto_cents, item.total_linea_cents)
                    } else {
                        let base = neto.max(0);
                        let iva = (base as f64 * tasa_iva).round() as i64;
                        (base, iva, base + iva)
                    };
                json!({
                    "sku": item.sku,
                    "cantidad": item.cantidad,
                    "base_cents": base_linea,
                    "iva_cents": iva_linea,
                    "total_cents": total_linea,
                })
            })
            .collect();
        let datos_cliente = json!({
            "parte_id": dto.parte_id,
            "razon_social": dto.razon_social,
            "ruc": dto.ruc,
            "emisor_ruc": serie.ruc_emisor,
            "orden_id": dto.orden_id,
            "items": items_fiscales,
        });

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO finance.comprobantes
               (id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id, base_gravada_cents,
                iva_cents, exento_cents, total_cents, moneda, estado, datos_cliente,
                emitido_en)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, 0, $10, $11, $12, $13,
                     CASE WHEN $12 = 'EMITIDO' THEN NOW() ELSE NULL END)
             RETURNING id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id, base_gravada_cents,
                       iva_cents, exento_cents, total_cents, moneda, estado,
                       datos_cliente, emitido_en, creado_en,
                       electronica_estado, electronica_referencia, electronica_enviada_en",
        )
        .bind(&serie.serie)
        .bind(&numero)
        .bind(&dto.tipo)
        .bind(uuid::Uuid::parse_str(&dto.orden_id).map_err(|_| {
            ApiError::validacion(json!([
                {"campo": "orden_id", "restricciones": ["isUuid"]}
            ]))
        })?)
        .bind(
            orden
                .empresa_id
                .as_deref()
                .map(uuid::Uuid::parse_str)
                .transpose()
                .map_err(|_| ApiError::interno())?,
        )
        .bind(negocio_uuid)
        .bind(
            parte_id
                .as_deref()
                .map(uuid::Uuid::parse_str)
                .transpose()
                .map_err(|_| {
                    ApiError::validacion(json!([
                        {"campo": "parte_id", "restricciones": ["isUuid"]}
                    ]))
                })?,
        )
        .bind(base as i32)
        .bind(iva as i32)
        .bind(total as i32)
        .bind(&moneda)
        .bind(estado)
        .bind(sqlx::types::Json(&datos_cliente))
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        let comprobante = comprobante_de_fila(&fila);

        // el secuencial se consume siempre (también en BORRADOR) para no romper
        // la consecutividad exigida por la DGI al activar la facturación real.
        sqlx::query(
            "UPDATE finance.series_comprobantes
             SET secuencial_actual = secuencial_actual + 1 WHERE serie = $1",
        )
        .bind(&serie.serie)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;

        if estado == "EMITIDO" {
            self.contabilidad
                .registrar_en_transaccion(
                    &mut tx,
                    &ParametrosAsiento {
                        concepto: format!(
                            "IVA trasladado {} {} {}",
                            dto.tipo, comprobante.serie, comprobante.numero
                        ),
                        tipo: "AJUSTE".to_string(),
                        referencia_tipo: Some("comprobante".to_string()),
                        referencia_id: Some(comprobante.id.clone()),
                        fecha: None,
                        moneda: None,
                        creado_por: Some("sistema".to_string()),
                        detalles: vec![
                            DetalleAsiento {
                                cuenta_codigo: "2.1.2".to_string(),
                                debe_cents: iva,
                                haber_cents: 0,
                                concepto: Some("IVA trasladado".to_string()),
                                orden: None,
                            },
                            DetalleAsiento {
                                cuenta_codigo: "2.1.3".to_string(),
                                debe_cents: 0,
                                haber_cents: iva,
                                concepto: Some("IVA por pagar".to_string()),
                                orden: None,
                            },
                        ],
                    },
                )
                .await?;
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    COMPROBANTE_EMITIDO,
                    json!({
                        "comprobante_id": comprobante.id,
                        "serie": comprobante.serie,
                        "numero": comprobante.numero,
                        "tipo": dto.tipo,
                        "orden_id": dto.orden_id,
                        "parte_id": comprobante.parte_id,
                        "base_gravada_cents": base,
                        "iva_cents": iva,
                        "total_cents": total,
                        "moneda": "C$",
                    }),
                )
                .await?;
        }
        tx.commit().await.map_err(error_db)?;
        self.enviar_electronico(comprobante).await
    }

    /// Envía el comprobante al proveedor electrónico si el tenant lo activó.
    /// Un fallo del proveedor no invalida el comprobante: queda con
    /// `electronica_estado = error` para reintento operativo.
    async fn enviar_electronico(
        &self,
        mut comprobante: Comprobante,
    ) -> Result<Comprobante, ApiError> {
        if let Some(empresa_id) = comprobante.empresa_id.as_deref() {
            let configuracion = sqlx::query(
                "SELECT facturacion_electronica, proveedor_electronico, ambiente_electronico
                   FROM finance.configuracion_fiscal
                  WHERE empresa_id = $1::uuid
                    AND (negocio_id = $2::uuid OR negocio_id IS NULL)
                  ORDER BY (negocio_id IS NOT NULL) DESC LIMIT 1",
            )
            .bind(empresa_id)
            .bind(comprobante.negocio_id.as_deref())
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
            if let Some(fila) = configuracion {
                let electronica: bool = fila.try_get("facturacion_electronica").unwrap_or(false);
                if electronica {
                    let proveedor = fila
                        .try_get::<String, _>("proveedor_electronico")
                        .unwrap_or_else(|_| "simulado".to_string());
                    let ambiente = fila
                        .try_get::<String, _>("ambiente_electronico")
                        .unwrap_or_else(|_| "pruebas".to_string());
                    let resultado = if proveedor == "http" {
                        match self.http.as_ref() {
                            Some(http) => http.emitir(&comprobante, &ambiente).await,
                            None => Err(ApiError::dominio(
                                "PROVEEDOR_NO_CONFIGURADO",
                                "No hay URL de facturación electrónica configurada.",
                            )),
                        }
                    } else {
                        FacturacionSimulada.emitir(&comprobante, &ambiente).await
                    };
                    let (estado, referencia) = match resultado {
                        Ok(referencia) => (referencia.estado, referencia.referencia),
                        Err(_) => ("error".to_string(), String::new()),
                    };
                    sqlx::query(
                        "UPDATE finance.comprobantes
                            SET electronica_estado = $2, electronica_referencia = $3,
                                electronica_enviada_en = NOW()
                          WHERE id = $1::uuid",
                    )
                    .bind(&comprobante.id)
                    .bind(&estado)
                    .bind(&referencia)
                    .execute(self.db.pool())
                    .await
                    .map_err(error_db)?;
                    comprobante.electronica_estado = Some(estado);
                    comprobante.electronica_referencia =
                        Some(referencia).filter(|valor| !valor.is_empty());
                    comprobante.electronica_enviada_en = Some(crate::db::iso_ms(Utc::now()));
                }
            }
        }
        Ok(comprobante)
    }

    /// Emite una nota de crédito parcial con montos explícitos de una RMA.
    pub async fn emitir_nota_credito(&self, dto: NotaCreditoDto) -> Result<Comprobante, ApiError> {
        if dto.base_cents < 0
            || dto.iva_cents < 0
            || dto.total_cents <= 0
            || dto.base_cents.saturating_add(dto.iva_cents) != dto.total_cents
        {
            return Err(ApiError::dominio(
                "MONTO_NOTA_CREDITO_INVALIDO",
                "La nota de crédito requiere base + IVA = total.",
            ));
        }
        let negocio_uuid = dto
            .negocio_id
            .as_deref()
            .map(uuid::Uuid::parse_str)
            .transpose()
            .map_err(|_| ApiError::interno())?;
        let serie = sqlx::query(
            "SELECT serie, negocio_id, tipo, prefijo, secuencial_actual, ruc_emisor,
                    jurisdiccion, activa
               FROM finance.series_comprobantes
              WHERE tipo = 'NOTA_CREDITO' AND activa
                AND (negocio_id = $1::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC, serie
              LIMIT 1",
        )
        .bind(negocio_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "SERIE_NO_DISPONIBLE",
                "No hay serie activa para NOTA_CREDITO.",
            )
        })?;
        let serie = serie_de_fila(&serie);
        let moneda = sqlx::query_scalar::<_, String>(
            "SELECT moneda FROM finance.jurisdicciones WHERE codigo_pais = $1",
        )
        .bind(&serie.jurisdiccion)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .or(dto.moneda.clone())
        .unwrap_or_else(|| "NIO".to_string());
        let numero = format!("{}{:08}", serie.prefijo, serie.secuencial_actual + 1);
        let estado = if self.emitir { "EMITIDO" } else { "BORRADOR" };
        let datos_cliente = json!({
            "devolucion_id": dto.devolucion_id,
            "razon_social": Value::Null,
            "ruc": Value::Null,
            "emisor_ruc": serie.ruc_emisor,
            "orden_id": dto.orden_id,
            "items": dto.items,
        });

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO finance.comprobantes
               (id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id,
                base_gravada_cents, iva_cents, exento_cents, total_cents, moneda,
                estado, datos_cliente, emitido_en)
             VALUES (gen_random_uuid(), $1, $2, 'NOTA_CREDITO', $3::uuid, $4::uuid,
                     $5::uuid, $6::uuid, $7, $8, 0, $9, $10, $11, $12,
                     CASE WHEN $11 = 'EMITIDO' THEN NOW() ELSE NULL END)
             RETURNING id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id,
                       base_gravada_cents, iva_cents, exento_cents, total_cents, moneda,
                       estado, datos_cliente, emitido_en, creado_en,
                       electronica_estado, electronica_referencia, electronica_enviada_en",
        )
        .bind(&serie.serie)
        .bind(&numero)
        .bind(
            uuid::Uuid::parse_str(&dto.orden_id)
                .map_err(|_| ApiError::validacion(json!([{"campo": "orden_id"}])))?,
        )
        .bind(
            uuid::Uuid::parse_str(&dto.empresa_id)
                .map_err(|_| ApiError::validacion(json!([{"campo": "empresa_id"}])))?,
        )
        .bind(negocio_uuid)
        .bind(
            dto.parte_id
                .as_deref()
                .map(uuid::Uuid::parse_str)
                .transpose()
                .map_err(|_| ApiError::interno())?,
        )
        .bind(dto.base_cents as i32)
        .bind(dto.iva_cents as i32)
        .bind(dto.total_cents as i32)
        .bind(&moneda)
        .bind(estado)
        .bind(sqlx::types::Json(&datos_cliente))
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        let comprobante = comprobante_de_fila(&fila);
        sqlx::query(
            "UPDATE finance.series_comprobantes
             SET secuencial_actual = secuencial_actual + 1 WHERE serie = $1",
        )
        .bind(&serie.serie)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;

        if estado == "EMITIDO" && dto.iva_cents > 0 {
            self.contabilidad
                .registrar_en_transaccion(
                    &mut tx,
                    &ParametrosAsiento {
                        concepto: format!("IVA revertido nota de credito {}", comprobante.numero),
                        tipo: "AJUSTE".to_string(),
                        referencia_tipo: Some("comprobante".to_string()),
                        referencia_id: Some(comprobante.id.clone()),
                        fecha: None,
                        moneda: None,
                        creado_por: Some("sistema".to_string()),
                        detalles: vec![
                            DetalleAsiento {
                                cuenta_codigo: "2.1.3".to_string(),
                                debe_cents: dto.iva_cents,
                                haber_cents: 0,
                                concepto: Some("IVA por pagar revertido".to_string()),
                                orden: None,
                            },
                            DetalleAsiento {
                                cuenta_codigo: "2.1.2".to_string(),
                                debe_cents: 0,
                                haber_cents: dto.iva_cents,
                                concepto: Some("IVA trasladado revertido".to_string()),
                                orden: None,
                            },
                        ],
                    },
                )
                .await?;
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    COMPROBANTE_EMITIDO,
                    json!({
                        "comprobante_id": comprobante.id,
                        "serie": comprobante.serie,
                        "numero": comprobante.numero,
                        "tipo": "NOTA_CREDITO",
                        "orden_id": dto.orden_id,
                        "devolucion_id": dto.devolucion_id,
                        "base_gravada_cents": dto.base_cents,
                        "iva_cents": dto.iva_cents,
                        "total_cents": dto.total_cents,
                        "moneda": moneda,
                    }),
                )
                .await?;
        }
        tx.commit().await.map_err(error_db)?;
        self.enviar_electronico(comprobante).await
    }

    /// Exporta los datos fiscales del tenant. No incluye bytes de PDF.
    pub async fn exportar_tenant(
        &self,
        empresa_id: &str,
        negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Result<Value, ApiError> {
        let empresa = uuid::Uuid::parse_str(empresa_id)
            .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser UUID."))?;
        let negocios: Vec<uuid::Uuid> = negocio_ids
            .iter()
            .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
            .collect();
        let usuarios: Vec<uuid::Uuid> = usuario_ids
            .iter()
            .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
            .collect();
        let mut datos = Map::new();
        let comprobantes: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY c.creado_en), '[]'::jsonb)
               FROM finance.comprobantes c WHERE c.empresa_id = $1",
        )
        .bind(empresa)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("comprobantes".into(), comprobantes);
        let pdfs: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(jsonb_build_object(
                        'comprobante_id', p.comprobante_id,
                        'objeto_id', p.objeto_id,
                        'generado_en', p.generado_en,
                        'contenido_base64', encode(p.contenido, 'base64')
                    )), '[]'::jsonb)
               FROM finance.comprobante_pdf p
              WHERE p.comprobante_id IN (
                  SELECT id FROM finance.comprobantes WHERE empresa_id = $1
              )",
        )
        .bind(empresa)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("comprobantes_pdf".into(), pdfs);
        for (nombre, sql) in [
            (
                "configuracion_fiscal",
                "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
                   FROM finance.configuracion_fiscal t WHERE t.empresa_id = $1",
            ),
            (
                "credito_cuentas",
                "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
                   FROM finance.credito_cuentas t WHERE t.empresa_id = $1",
            ),
            (
                "workspace_registros",
                "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
                   FROM finance.workspace_registros t WHERE t.tenant_id = $1",
            ),
        ] {
            let valor: Value = sqlx::query_scalar(sql)
                .bind(empresa)
                .fetch_one(self.db.pool())
                .await
                .map_err(error_db)?;
            datos.insert(nombre.into(), valor);
        }
        let series: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM finance.series_comprobantes t WHERE t.negocio_id = ANY($1::uuid[])",
        )
        .bind(&negocios)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("series_comprobantes".into(), series);
        let asientos: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(a) ORDER BY a.creado_en), '[]'::jsonb)
               FROM finance.asientos a
              WHERE a.referencia_tipo = 'comprobante'
                AND a.referencia_id IN (
                    SELECT id::text FROM finance.comprobantes WHERE empresa_id = $1
                )",
        )
        .bind(empresa)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("asientos".into(), asientos);
        let metricas: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM finance.metricas_vendedores t WHERE t.vendedor_id = ANY($1::uuid[])",
        )
        .bind(&usuarios)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        datos.insert("metricas_vendedores".into(), metricas);
        Ok(Value::Object(datos))
    }

    /// Anonimiza los datos personales del tenant en finanzas y elimina su
    /// configuración fiscal. Los comprobantes y asientos se conservan por
    /// retención contable.
    pub async fn purgar_tenant(
        &self,
        empresa_id: &str,
        _negocio_ids: &[String],
        usuario_ids: &[String],
    ) -> Result<Value, ApiError> {
        let empresa = uuid::Uuid::parse_str(empresa_id)
            .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser UUID."))?;
        let usuarios: Vec<uuid::Uuid> = usuario_ids
            .iter()
            .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
            .collect();
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        sqlx::query(
            "UPDATE finance.comprobantes
                SET datos_cliente = jsonb_build_object('redactado', true),
                    parte_id = NULL
              WHERE empresa_id = $1",
        )
        .bind(empresa)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        sqlx::query(
            "DELETE FROM finance.comprobante_pdf
              WHERE comprobante_id IN (
                  SELECT id FROM finance.comprobantes WHERE empresa_id = $1
              )",
        )
        .bind(empresa)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        sqlx::query("DELETE FROM finance.configuracion_fiscal WHERE empresa_id = $1")
            .bind(empresa)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        sqlx::query("DELETE FROM finance.credito_cuentas WHERE empresa_id = $1")
            .bind(empresa)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        sqlx::query("DELETE FROM finance.workspace_registros WHERE tenant_id = $1")
            .bind(empresa)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        // Las series se conservan: los comprobantes retenidos las referencian.
        sqlx::query("DELETE FROM finance.metricas_vendedores WHERE vendedor_id = ANY($1::uuid[])")
            .bind(&usuarios)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        Ok(json!({ "empresa_id": empresa_id, "anonimizado": true }))
    }

    /// Indica si el tenant tiene PDF habilitado y devuelve el comprobante.
    pub async fn pdf_de(
        &self,
        id: &str,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Option<Vec<u8>>, ApiError> {
        let habilitado: Option<bool> = sqlx::query_scalar(
            "SELECT formato_pdf FROM finance.configuracion_fiscal
              WHERE empresa_id = $1::uuid AND (negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC LIMIT 1",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if habilitado != Some(true) {
            return Err(ApiError::dominio(
                "PDF_NO_ACTIVADO",
                "La facturación PDF no está activada para este negocio.",
            ));
        }
        let comprobante_id = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El comprobante debe ser un UUID."))?;
        let guardado: Option<(Option<Vec<u8>>, Option<uuid::Uuid>)> = sqlx::query_as(
            "SELECT contenido, objeto_id FROM finance.comprobante_pdf WHERE comprobante_id = $1",
        )
        .bind(comprobante_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some((contenido, objeto_id)) = &guardado {
            if let Some(pdf) = contenido {
                return Ok(Some(pdf.clone()));
            }
            if let (Some(objeto_id), Some(almacen)) = (objeto_id, self.pdf_almacen.as_ref()) {
                if let Ok(pdf) = almacen.leer(empresa_id, &objeto_id.to_string()).await {
                    return Ok(Some(pdf));
                }
            }
        }
        let comprobante = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Comprobante", id))?;
        let pdf = crate::pdf::comprobante_pdf(&comprobante);
        let mut objeto_id = None;
        let mut contenido = Some(pdf.clone());
        if let Some(almacen) = self.pdf_almacen.as_ref() {
            let nombre = format!("comprobante-{}.pdf", comprobante.numero);
            if let Ok(id) = almacen.subir(empresa_id, &nombre, &pdf).await {
                if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
                    objeto_id = Some(uuid);
                    contenido = None;
                }
            }
        }
        sqlx::query(
            "INSERT INTO finance.comprobante_pdf (comprobante_id, contenido, objeto_id)
             VALUES ($1, $2, $3) ON CONFLICT (comprobante_id) DO NOTHING",
        )
        .bind(comprobante_id)
        .bind(&contenido)
        .bind(objeto_id)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        // En una carrera, devolvemos la copia persistida canónica.
        let guardado: Option<(Option<Vec<u8>>, Option<uuid::Uuid>)> = sqlx::query_as(
            "SELECT contenido, objeto_id FROM finance.comprobante_pdf WHERE comprobante_id = $1",
        )
        .bind(comprobante_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some((Some(pdf), _)) = guardado {
            return Ok(Some(pdf));
        }
        if let Some((_, Some(objeto_id))) = guardado {
            if let Some(almacen) = self.pdf_almacen.as_ref() {
                if let Ok(pdf) = almacen.leer(empresa_id, &objeto_id.to_string()).await {
                    return Ok(Some(pdf));
                }
            }
        }
        Ok(Some(pdf))
    }

    pub async fn obtener(&self, id: &str) -> Result<Option<Comprobante>, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El comprobante debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT id, serie, numero, tipo, orden_id, empresa_id, negocio_id, parte_id,
                    base_gravada_cents, iva_cents, exento_cents, total_cents, moneda,
                    estado, datos_cliente, emitido_en, creado_en,
                    electronica_estado, electronica_referencia, electronica_enviada_en
               FROM finance.comprobantes WHERE id = $1::uuid",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(comprobante_de_fila))
    }

    /// Anula un comprobante (auditoría: el número emitido no se reutiliza).
    pub async fn anular(&self, id: &str) -> Result<Option<Comprobante>, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El comprobante debe ser un UUID."))?;
        let fila = sqlx::query(
            "UPDATE finance.comprobantes SET estado = 'ANULADO'
             WHERE id = $1 AND estado IN ('BORRADOR', 'EMITIDO')
             RETURNING id, serie, numero, tipo, orden_id, empresa_id, parte_id, base_gravada_cents,
                       iva_cents, exento_cents, total_cents, moneda, estado,
                       datos_cliente, emitido_en, creado_en,
                       electronica_estado, electronica_referencia, electronica_enviada_en",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(comprobante_de_fila))
    }
}

fn comprobante_de_fila(fila: &sqlx::postgres::PgRow) -> Comprobante {
    Comprobante {
        id: uuid_texto(fila, "id").unwrap_or_default(),
        serie: fila.try_get("serie").unwrap_or_default(),
        numero: fila.try_get("numero").unwrap_or_default(),
        tipo: fila.try_get("tipo").unwrap_or_default(),
        orden_id: uuid_texto(fila, "orden_id").ok(),
        empresa_id: uuid_texto(fila, "empresa_id").ok(),
        negocio_id: uuid_texto(fila, "negocio_id").ok(),
        parte_id: uuid_texto(fila, "parte_id").ok(),
        base_gravada_cents: fila.try_get::<i32, _>("base_gravada_cents").unwrap_or(0) as i64,
        iva_cents: fila.try_get::<i32, _>("iva_cents").unwrap_or(0) as i64,
        exento_cents: fila.try_get::<i32, _>("exento_cents").unwrap_or(0) as i64,
        total_cents: fila.try_get::<i32, _>("total_cents").unwrap_or(0) as i64,
        moneda: fila.try_get("moneda").unwrap_or_default(),
        estado: fila.try_get("estado").unwrap_or_default(),
        datos_cliente: jsonb(fila, "datos_cliente").ok(),
        emitido_en: fila
            .try_get::<Option<DateTime<Utc>>, _>("emitido_en")
            .ok()
            .flatten()
            .map(crate::db::iso_ms),
        creado_en: timestamp(fila, "creado_en").unwrap_or_default(),
        electronica_estado: fila.try_get("electronica_estado").ok(),
        electronica_referencia: fila.try_get("electronica_referencia").ok(),
        electronica_enviada_en: fila
            .try_get::<Option<DateTime<Utc>>, _>("electronica_enviada_en")
            .ok()
            .flatten()
            .map(crate::db::iso_ms),
    }
}

fn serie_de_fila(fila: &sqlx::postgres::PgRow) -> SerieComprobante {
    SerieComprobante {
        serie: fila.try_get("serie").unwrap_or_default(),
        negocio_id: uuid_texto(fila, "negocio_id").ok(),
        tipo: fila.try_get("tipo").unwrap_or_default(),
        prefijo: fila.try_get("prefijo").unwrap_or_default(),
        secuencial_actual: fila.try_get::<i32, _>("secuencial_actual").unwrap_or(0) as i64,
        ruc_emisor: fila.try_get("ruc_emisor").unwrap_or_default(),
        jurisdiccion: fila.try_get("jurisdiccion").unwrap_or_default(),
        activa: fila.try_get("activa").unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OrdenesFijas;

    #[async_trait]
    impl Ordenes for OrdenesFijas {
        async fn consultar(&self, order_id: &str) -> Option<OrdenResumen> {
            (order_id == "orden-1").then_some(OrdenResumen {
                total_cents: 100_000,
                empresa_id: None,
                negocio_id: None,
                items: Vec::new(),
                descuento_cupon_cents: 0,
            })
        }
    }

    #[tokio::test]
    async fn debe_consultar_la_orden_por_la_superficie_interna() {
        let ordenes = OrdenesFijas;
        assert_eq!(
            ordenes
                .consultar("orden-1")
                .await
                .map(|orden| orden.total_cents),
            Some(100_000)
        );
        assert!(ordenes.consultar("otra").await.is_none());
    }

    #[test]
    fn debe_calcular_la_base_y_el_iva_del_comprobante() {
        let base = money::desde_centavos(100_000).expect("base");
        let iva = money::multiplicar_por(base, 0.15).expect("iva");
        assert_eq!(iva, 15_000);
        assert_eq!(money::sumar(base, iva).expect("total"), 115_000);
        assert_eq!(money::string(115_000), "1150.00");
    }
}
