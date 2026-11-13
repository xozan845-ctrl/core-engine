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
use serde_json::{json, Value};
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
    pub parte_id: Option<String>,
    pub base_gravada_cents: i64,
    pub iva_cents: i64,
    pub exento_cents: i64,
    pub total_cents: i64,
    pub moneda: String,
    pub estado: String,
    pub datos_cliente: Option<Value>,
    pub emitido_en: Option<String>,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerieComprobante {
    pub serie: String,
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
pub struct OrdenResumen {
    pub total_cents: i64,
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
        respuesta.json::<OrdenResumen>().await.ok()
    }
}

pub struct SinOrdenes;

#[async_trait]
impl Ordenes for SinOrdenes {
    async fn consultar(&self, _order_id: &str) -> Option<OrdenResumen> {
        None
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

pub struct Facturacion {
    db: Arc<Db>,
    outbox: Outbox,
    contabilidad: Arc<Contabilidad>,
    ordenes: Arc<dyn Ordenes>,
    emitir: bool,
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
        }
    }

    pub async fn listar(
        &self,
        desde: Option<DateTime<Utc>>,
        hasta: Option<DateTime<Utc>>,
        estado: Option<&str>,
    ) -> Result<Vec<Comprobante>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, serie, numero, tipo, orden_id, parte_id, base_gravada_cents,
                    iva_cents, exento_cents, total_cents, moneda, estado,
                    datos_cliente, emitido_en, creado_en
             FROM finance.comprobantes
             WHERE ($1::timestamptz IS NULL OR creado_en >= $1)
               AND ($2::timestamptz IS NULL OR creado_en <= $2)
               AND ($3::text IS NULL OR estado = $3)
             ORDER BY creado_en DESC
             LIMIT 300",
        )
        .bind(desde)
        .bind(hasta)
        .bind(estado)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(comprobante_de_fila).collect())
    }

    pub async fn series(&self) -> Result<Vec<SerieComprobante>, ApiError> {
        let filas = sqlx::query(
            "SELECT serie, tipo, prefijo, secuencial_actual, ruc_emisor, jurisdiccion, activa
             FROM finance.series_comprobantes ORDER BY serie",
        )
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

        let serie = sqlx::query(
            "SELECT serie, tipo, prefijo, secuencial_actual, ruc_emisor, jurisdiccion, activa
             FROM finance.series_comprobantes WHERE tipo = $1 AND activa ORDER BY serie LIMIT 1",
        )
        .bind(&dto.tipo)
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

        let tasa_iva: Option<f64> = sqlx::query_scalar(
            "SELECT tasa_iva FROM finance.jurisdicciones WHERE codigo_pais = $1",
        )
        .bind(&serie.jurisdiccion)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let tasa_iva = tasa_iva.unwrap_or(0.15);
        let base = money::desde_centavos(orden.total_cents)?;
        let iva = money::multiplicar_por(base, tasa_iva)?;
        let total = money::sumar(base, iva)?;
        let numero = format!("{}{:08}", serie.prefijo, serie.secuencial_actual + 1);
        let estado = if self.emitir { "EMITIDO" } else { "BORRADOR" };
        let parte_id = dto.parte_id.clone();
        let datos_cliente = json!({
            "parte_id": dto.parte_id,
            "razon_social": dto.razon_social,
            "ruc": dto.ruc,
        });

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO finance.comprobantes
               (id, serie, numero, tipo, orden_id, parte_id, base_gravada_cents,
                iva_cents, exento_cents, total_cents, moneda, estado, datos_cliente,
                emitido_en)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, 0, $8, 'C$', $9, $10,
                     CASE WHEN $9 = 'EMITIDO' THEN NOW() ELSE NULL END)
             RETURNING id, serie, numero, tipo, orden_id, parte_id, base_gravada_cents,
                       iva_cents, exento_cents, total_cents, moneda, estado,
                       datos_cliente, emitido_en, creado_en",
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
        Ok(comprobante)
    }

    /// Anula un comprobante (auditoría: el número emitido no se reutiliza).
    pub async fn anular(&self, id: &str) -> Result<Option<Comprobante>, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El comprobante debe ser un UUID."))?;
        let fila = sqlx::query(
            "UPDATE finance.comprobantes SET estado = 'ANULADO'
             WHERE id = $1 AND estado IN ('BORRADOR', 'EMITIDO')
             RETURNING id, serie, numero, tipo, orden_id, parte_id, base_gravada_cents,
                       iva_cents, exento_cents, total_cents, moneda, estado,
                       datos_cliente, emitido_en, creado_en",
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
    }
}

fn serie_de_fila(fila: &sqlx::postgres::PgRow) -> SerieComprobante {
    SerieComprobante {
        serie: fila.try_get("serie").unwrap_or_default(),
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
