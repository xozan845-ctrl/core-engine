//! Dominio de inteligencia de mercado: consultas analíticas, ingesta y
//! agregación de ventas geolocalizadas.
//!
//! Réplica de `InteligenciaRepository`, `InteligenciaService` y del SQL del
//! TypeScript, incluyendo el formato de fila de `node-postgres` (bigint y
//! numeric viajan como string; `date`/`timestamptz` como ISO 8601).

use crate::db::{
    fecha, float8, float8_opcional, int4, texto, texto_opcional, timestamp, timestamp_opcional,
    uuid_opcional, uuid_texto, Db,
};
use crate::errors::ApiError;
use crate::logs;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filtros {
    pub desde: Option<String>,
    pub hasta: Option<String>,
    pub sku: Option<String>,
    pub vendedor_id: Option<String>,
    pub cursor: Option<String>,
    pub limite: Option<i32>,
    pub centro_lat: Option<f64>,
    pub centro_lng: Option<f64>,
    pub radio_metros: Option<f64>,
    pub lat_min: Option<f64>,
    pub lat_max: Option<f64>,
    pub lng_min: Option<f64>,
    pub lng_max: Option<f64>,
    pub empresa_id: Option<String>,
    pub fecha_inicio: Option<String>,
    pub fecha_fin: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct VentaGeolocalizada {
    pub order_id: String,
    pub vendedor_id: String,
    pub skus: Vec<String>,
    pub monto_cents: i64,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lng: Option<f64>,
    #[serde(default)]
    pub precision: Option<f64>,
    #[serde(default)]
    pub velocidad: Option<f64>,
    #[serde(default)]
    pub rumbo: Option<f64>,
    #[serde(default)]
    pub tipo_actividad: Option<String>,
    #[serde(default)]
    pub resultado_visita: Option<String>,
    #[serde(default)]
    pub distancia_cliente_metros: Option<f64>,
    #[serde(default)]
    pub rango_edad: Option<String>,
    #[serde(default)]
    pub genero: Option<String>,
}

#[derive(Clone)]
pub struct Inteligencia {
    pub(crate) db: Arc<Db>,
}

impl Inteligencia {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn obtener_mapa_calor(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT ST_Y(geom) AS lat, ST_X(geom) AS lng, peso, tipo
             FROM intelligence.puntos_calor
             WHERE deleted_at IS NULL AND geom IS NOT NULL",
        );
        if let Some(vendedor) = filtros.vendedor_id.as_deref() {
            qb.push(" AND vendedor_id = ")
                .push_bind(vendedor)
                .push("::uuid");
        }
        if let Some(sku) = filtros.sku.as_deref() {
            qb.push(" AND sku = ").push_bind(sku);
        }
        if let (Some(lat), Some(lng), Some(radio)) =
            (filtros.centro_lat, filtros.centro_lng, filtros.radio_metros)
        {
            qb.push(" AND ST_DWithin(geom, ST_SetSRID(ST_MakePoint(")
                .push_bind(lng)
                .push(", ")
                .push_bind(lat)
                .push("), 4326)::geography, ")
                .push_bind(radio)
                .push(")");
        }
        let filas = qb
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut puntos = Vec::with_capacity(filas.len());
        for fila in &filas {
            puntos.push(json!({
                "lat": float8(fila, "lat").map_err(error_db)?,
                "lng": float8(fila, "lng").map_err(error_db)?,
                "peso": int4(fila, "peso").map_err(error_db)?,
                "tipo": texto(fila, "tipo").map_err(error_db)?,
            }));
        }
        Ok(json!({
            "puntos": puntos,
            "total_puntos": filas.len(),
        }))
    }

    pub async fn obtener_rendimiento(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        let limite = filtros.limite.unwrap_or(50);
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT vendedor_id, total_ventas, total_monto_cents::text AS total_monto_cents,
                    ticket_promedio_cents::text AS ticket_promedio_cents,
                    pedidos_entregados, tasa_efectividad::text AS tasa_efectividad,
                    actualizado_en
             FROM intelligence.rendimiento_vendedor WHERE 1=1",
        );
        if let Some(vendedor) = filtros.vendedor_id.as_deref() {
            qb.push(" AND vendedor_id = ")
                .push_bind(vendedor)
                .push("::uuid");
        }
        if let Some(cursor) = filtros.cursor.as_deref() {
            if let Some((ventas, vendedor)) = cursor_par(cursor) {
                let total = parse_int_js(&ventas).ok_or_else(ApiError::interno)?;
                qb.push(" AND (total_ventas, vendedor_id) < (")
                    .push_bind(total)
                    .push(", ")
                    .push_bind(vendedor)
                    .push("::uuid)");
            }
        }
        qb.push(" ORDER BY total_ventas DESC, vendedor_id DESC LIMIT ")
            .push_bind(limite);
        let filas = qb
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut data = Vec::with_capacity(filas.len());
        for fila in &filas {
            data.push(json!({
                "vendedor_id": uuid_texto(fila, "vendedor_id").map_err(error_db)?,
                "total_ventas": int4(fila, "total_ventas").map_err(error_db)?,
                "total_monto_cents": texto(fila, "total_monto_cents").map_err(error_db)?,
                "ticket_promedio_cents": texto(fila, "ticket_promedio_cents").map_err(error_db)?,
                "pedidos_entregados": int4(fila, "pedidos_entregados").map_err(error_db)?,
                "tasa_efectividad": texto(fila, "tasa_efectividad").map_err(error_db)?,
                "actualizado_en": timestamp(fila, "actualizado_en").map_err(error_db)?,
            }));
        }
        let next_cursor = if filas.len() == limite as usize && !filas.is_empty() {
            let ultima = filas.last().expect("fila");
            let ventas = int4(ultima, "total_ventas").map_err(error_db)?;
            let vendedor = uuid_texto(ultima, "vendedor_id").map_err(error_db)?;
            Some(format!("{ventas}_{vendedor}"))
        } else {
            None
        };
        Ok(json!({ "data": data, "next_cursor": next_cursor }))
    }

    pub async fn obtener_cobertura(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        let limite = filtros.limite.unwrap_or(100);
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT lat_cell, lng_cell, total_ventas, total_monto_cents::text AS total_monto_cents,
                    total_vendedores, actualizado_en,
                    ST_AsGeoJSON(bounding_box) AS geojson
             FROM intelligence.cobertura_zona WHERE 1=1",
        );
        if let (Some(lat_min), Some(lat_max), Some(lng_min), Some(lng_max)) = (
            filtros.lat_min,
            filtros.lat_max,
            filtros.lng_min,
            filtros.lng_max,
        ) {
            qb.push(" AND lat_cell BETWEEN ")
                .push_bind(lat_min)
                .push(" AND ")
                .push_bind(lat_max)
                .push(" AND lng_cell BETWEEN ")
                .push_bind(lng_min)
                .push(" AND ")
                .push_bind(lng_max);
        }
        qb.push(" ORDER BY total_ventas DESC LIMIT ")
            .push_bind(limite);
        let filas = qb
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut data = Vec::with_capacity(filas.len());
        for fila in &filas {
            data.push(json!({
                "lat_cell": float8(fila, "lat_cell").map_err(error_db)?,
                "lng_cell": float8(fila, "lng_cell").map_err(error_db)?,
                "total_ventas": int4(fila, "total_ventas").map_err(error_db)?,
                "total_monto_cents": texto(fila, "total_monto_cents").map_err(error_db)?,
                "total_vendedores": int4(fila, "total_vendedores").map_err(error_db)?,
                "actualizado_en": timestamp(fila, "actualizado_en").map_err(error_db)?,
                "geojson": texto_opcional(fila, "geojson").map_err(error_db)?,
            }));
        }
        Ok(Value::Array(data))
    }

    pub async fn obtener_demanda(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        let limite = filtros.limite.unwrap_or(50);
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT empresa_id, sku, nombre, total_vendido,
                    total_monto_cents::text AS total_monto_cents, stock_actual,
                    ultima_venta_en, actualizado_en
             FROM intelligence.metricas_producto WHERE 1=1",
        );
        if let Some(empresa) = filtros.empresa_id.as_deref() {
            qb.push(" AND empresa_id = ")
                .push_bind(empresa)
                .push("::uuid");
        }
        if let Some(sku) = filtros.sku.as_deref() {
            qb.push(" AND sku = ").push_bind(sku);
        }
        if let Some(cursor) = filtros.cursor.as_deref() {
            if let Some((vendido, sku)) = cursor_par(cursor) {
                let total = parse_int_js(&vendido).ok_or_else(ApiError::interno)?;
                qb.push(" AND (total_vendido, sku) < (")
                    .push_bind(total)
                    .push(", ")
                    .push_bind(sku)
                    .push(")");
            }
        }
        qb.push(" ORDER BY total_vendido DESC, sku DESC LIMIT ")
            .push_bind(limite);
        let filas = qb
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut data = Vec::with_capacity(filas.len());
        for fila in &filas {
            data.push(json!({
                "empresa_id": uuid_opcional(fila, "empresa_id").map_err(error_db)?,
                "sku": texto(fila, "sku").map_err(error_db)?,
                "nombre": texto(fila, "nombre").map_err(error_db)?,
                "total_vendido": int4(fila, "total_vendido").map_err(error_db)?,
                "total_monto_cents": texto(fila, "total_monto_cents").map_err(error_db)?,
                "stock_actual": int4(fila, "stock_actual").map_err(error_db)?,
                "ultima_venta_en": timestamp_opcional(fila, "ultima_venta_en").map_err(error_db)?,
                "actualizado_en": timestamp(fila, "actualizado_en").map_err(error_db)?,
            }));
        }
        let next_cursor = if filas.len() == limite as usize && !filas.is_empty() {
            let ultima = filas.last().expect("fila");
            let vendido = int4(ultima, "total_vendido").map_err(error_db)?;
            let sku = texto(ultima, "sku").map_err(error_db)?;
            Some(format!("{vendido}_{sku}"))
        } else {
            None
        };
        Ok(json!({ "data": data, "next_cursor": next_cursor }))
    }

    pub async fn obtener_resumen(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        if let Some(vendedor) = filtros.vendedor_id.as_deref() {
            let fila = sqlx::query(
                "SELECT total_pedidos::text AS total_pedidos,
                        ingresos_totales_cents::text AS ingresos_totales_cents,
                        productos_distintos::text AS productos_distintos,
                        calculado_en
                 FROM intelligence.mv_resumen_ejecutivo
                 WHERE vendedor_id = $1::uuid",
            )
            .bind(vendedor)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
            return match fila {
                Some(fila) => Ok(json!({
                    "total_pedidos": texto(&fila, "total_pedidos").map_err(error_db)?,
                    "ingresos_totales_cents": texto(&fila, "ingresos_totales_cents").map_err(error_db)?,
                    "productos_distintos": texto(&fila, "productos_distintos").map_err(error_db)?,
                    "calculado_en": timestamp(&fila, "calculado_en").map_err(error_db)?,
                })),
                None => Ok(Value::Null),
            };
        }

        let fila = sqlx::query(
            "SELECT SUM(total_pedidos)::text AS total_pedidos,
                    SUM(ingresos_totales_cents)::text AS ingresos_totales_cents,
                    COUNT(DISTINCT vendedor_id)::text AS vendedores_activos,
                    MAX(calculado_en) AS calculado_en
             FROM intelligence.mv_resumen_ejecutivo",
        )
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        match fila {
            Some(fila) => Ok(json!({
                "total_pedidos": texto_opcional(&fila, "total_pedidos").map_err(error_db)?,
                "ingresos_totales_cents": texto_opcional(&fila, "ingresos_totales_cents").map_err(error_db)?,
                "vendedores_activos": texto(&fila, "vendedores_activos").map_err(error_db)?,
                "calculado_en": timestamp_opcional(&fila, "calculado_en").map_err(error_db)?,
            })),
            None => Ok(Value::Null),
        }
    }

    pub async fn obtener_tendencias(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT fecha, sku, vendedor_id, total_ventas::text AS total_ventas,
                    total_monto_cents::text AS total_monto_cents,
                    dia_semana::text AS dia_semana
             FROM intelligence.mv_tendencias WHERE 1=1",
        );
        if let Some(desde) = filtros.fecha_inicio.as_deref() {
            qb.push(" AND fecha >= ").push_bind(desde).push("::date");
        }
        if let Some(hasta) = filtros.fecha_fin.as_deref() {
            qb.push(" AND fecha <= ").push_bind(hasta).push("::date");
        }
        if let Some(sku) = filtros.sku.as_deref() {
            qb.push(" AND sku = ").push_bind(sku);
        }
        if let Some(vendedor) = filtros.vendedor_id.as_deref() {
            qb.push(" AND vendedor_id = ")
                .push_bind(vendedor)
                .push("::uuid");
        }
        qb.push(" ORDER BY fecha ASC");
        let filas = qb
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut data = Vec::with_capacity(filas.len());
        for fila in &filas {
            data.push(json!({
                "fecha": fecha(fila, "fecha").map_err(error_db)?,
                "sku": texto(fila, "sku").map_err(error_db)?,
                "vendedor_id": uuid_texto(fila, "vendedor_id").map_err(error_db)?,
                "total_ventas": texto(fila, "total_ventas").map_err(error_db)?,
                "total_monto_cents": texto(fila, "total_monto_cents").map_err(error_db)?,
                "dia_semana": texto(fila, "dia_semana").map_err(error_db)?,
            }));
        }
        Ok(Value::Array(data))
    }

    pub async fn obtener_forecast(&self, sku: Option<&str>, dias: i32) -> Result<Value, ApiError> {
        let filas = sqlx::query(
            "SELECT feature_name, feature_value::text AS feature_value, computed_at
             FROM intelligence.feature_store
             WHERE entity_type = 'sku' AND entity_id = $1
               AND feature_name IN ('ventas_avg_7d', 'ventas_std_7d')",
        )
        .bind(sku)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;

        let mut avg = 0.0;
        let mut std = 0.0;
        for fila in &filas {
            let nombre = texto(fila, "feature_name").map_err(error_db)?;
            let valor = parse_float_js(&texto(fila, "feature_value").map_err(error_db)?);
            if nombre == "ventas_avg_7d" {
                avg = valor;
            } else if nombre == "ventas_std_7d" {
                std = valor;
            }
        }
        let computed_at = filas
            .first()
            .map(|fila| timestamp(fila, "computed_at"))
            .transpose()
            .map_err(error_db)?;

        let proyeccion = avg * dias as f64;
        let intervalo = std * (dias as f64).sqrt();
        let mut respuesta = Map::new();
        if let Some(sku) = sku {
            respuesta.insert("sku".into(), json!(sku));
        }
        respuesta.insert("dias_proyectados".into(), json!(dias));
        respuesta.insert(
            "ventas_proyectadas".into(),
            json!(proyeccion.round() as i64),
        );
        respuesta.insert(
            "intervalo_inferior".into(),
            json!((proyeccion - intervalo).round().max(0.0) as i64),
        );
        respuesta.insert(
            "intervalo_superior".into(),
            json!((proyeccion + intervalo).round() as i64),
        );
        respuesta.insert("avg_diario_base".into(), json!(avg));
        respuesta.insert("std_diario_base".into(), json!(std));
        respuesta.insert("features_computadas_en".into(), json!(computed_at));
        respuesta.insert("metodologia".into(), json!("media_movil_ponderada_7d"));
        Ok(Value::Object(respuesta))
    }

    pub async fn obtener_anomalias(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        let limite = filtros.limite.unwrap_or(50);
        let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT order_id, sku, vendedor_id, monto_cents::text AS monto_cents,
                    lat, lng, ocurrido_en,
                    ROUND(media_monto::NUMERIC, 2)::text AS media_monto,
                    ROUND(std_monto::NUMERIC, 2)::text AS std_monto,
                    ROUND(z_score::NUMERIC, 3)::text AS z_score
             FROM intelligence.vw_anomalias_recientes WHERE 1=1",
        );
        if let Some(sku) = filtros.sku.as_deref() {
            qb.push(" AND sku = ").push_bind(sku);
        }
        if let Some(vendedor) = filtros.vendedor_id.as_deref() {
            qb.push(" AND vendedor_id = ")
                .push_bind(vendedor)
                .push("::uuid");
        }
        qb.push(" LIMIT ").push_bind(limite);
        let filas = qb
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut anomalias = Vec::with_capacity(filas.len());
        for fila in &filas {
            anomalias.push(json!({
                "order_id": uuid_texto(fila, "order_id").map_err(error_db)?,
                "sku": texto(fila, "sku").map_err(error_db)?,
                "vendedor_id": uuid_texto(fila, "vendedor_id").map_err(error_db)?,
                "monto_cents": texto(fila, "monto_cents").map_err(error_db)?,
                "lat": float8_opcional(fila, "lat").map_err(error_db)?,
                "lng": float8_opcional(fila, "lng").map_err(error_db)?,
                "ocurrido_en": timestamp(fila, "ocurrido_en").map_err(error_db)?,
                "media_monto": texto(fila, "media_monto").map_err(error_db)?,
                "std_monto": texto(fila, "std_monto").map_err(error_db)?,
                "z_score": texto(fila, "z_score").map_err(error_db)?,
            }));
        }
        Ok(json!({ "total": anomalias.len(), "anomalias": anomalias }))
    }

    pub async fn obtener_score(&self, vendedor_id: &str) -> Result<Option<Value>, ApiError> {
        let filas = sqlx::query(
            "SELECT feature_name, feature_value::text AS feature_value, computed_at
             FROM intelligence.feature_store
             WHERE entity_type = 'vendedor' AND entity_id = $1",
        )
        .bind(vendedor_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        if filas.is_empty() {
            return Ok(None);
        }

        let mut mapa: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for fila in &filas {
            let nombre = texto(fila, "feature_name").map_err(error_db)?;
            let valor = parse_float_js(&texto(fila, "feature_value").map_err(error_db)?);
            mapa.insert(nombre, valor);
        }
        let churn = mapa.get("churn_risk_score").copied().unwrap_or(0.0);
        let nivel = if churn >= 1.0 {
            "ALTO"
        } else if churn >= 0.5 {
            "MEDIO"
        } else {
            "BAJO"
        };
        let computed_at = timestamp(&filas[0], "computed_at").map_err(error_db)?;
        Ok(Some(json!({
            "vendedor_id": vendedor_id,
            "tasa_efectividad": mapa.get("tasa_efectividad").copied(),
            "churn_risk_score": churn,
            "churn_risk_nivel": nivel,
            "features_computadas_en": computed_at,
        })))
    }

    pub async fn obtener_calidad(&self, horas: i32) -> Result<Value, ApiError> {
        let invalidos = sqlx::query(
            "SELECT event_tipo, unnest(errores) AS error, COUNT(*)::text AS total
             FROM intelligence.invalid_events
             WHERE registrado_en >= NOW() - ($1::text || ' hours')::INTERVAL
             GROUP BY event_tipo, error
             ORDER BY total DESC
             LIMIT 50",
        )
        .bind(horas)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;

        let procesados = sqlx::query(
            "SELECT COUNT(*)::text AS total
             FROM intelligence.eventos_procesados
             WHERE procesado_en >= NOW() - ($1::text || ' hours')::INTERVAL",
        )
        .bind(horas)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;

        let mut total_invalidos = 0_i64;
        let mut errores = Vec::with_capacity(invalidos.len());
        for fila in &invalidos {
            let total = texto(fila, "total").map_err(error_db)?;
            total_invalidos += total.parse::<i64>().unwrap_or(0);
            errores.push(json!({
                "event_tipo": texto(fila, "event_tipo").map_err(error_db)?,
                "error": texto(fila, "error").map_err(error_db)?,
                "total": total,
            }));
        }
        let total_ok = procesados
            .as_ref()
            .map(|fila| texto(fila, "total"))
            .transpose()
            .map_err(error_db)?
            .and_then(|valor| valor.parse::<i64>().ok())
            .unwrap_or(0);
        let tasa = if total_ok + total_invalidos > 0 {
            let valor = (total_invalidos as f64 / (total_ok + total_invalidos) as f64) * 100.0;
            format!("{valor:.2}").parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };

        Ok(json!({
            "ventana_horas": horas,
            "eventos_procesados": total_ok,
            "eventos_invalidos": total_invalidos,
            "tasa_rechazo_pct": tasa,
            "errores_frecuentes": errores,
        }))
    }
}

#[async_trait]
pub trait InteligenciaPort: Send + Sync {
    async fn obtener_mapa_calor(&self, filtros: &Filtros) -> Result<Value, ApiError>;
    async fn obtener_rendimiento(&self, filtros: &Filtros) -> Result<Value, ApiError>;
    async fn obtener_cobertura(&self, filtros: &Filtros) -> Result<Value, ApiError>;
    async fn obtener_demanda(&self, filtros: &Filtros) -> Result<Value, ApiError>;
    async fn obtener_resumen(&self, filtros: &Filtros) -> Result<Value, ApiError>;
    async fn obtener_tendencias(&self, filtros: &Filtros) -> Result<Value, ApiError>;
    async fn obtener_forecast(&self, sku: Option<&str>, dias: i32) -> Result<Value, ApiError>;
    async fn obtener_anomalias(&self, filtros: &Filtros) -> Result<Value, ApiError>;
    async fn obtener_score(&self, vendedor_id: &str) -> Result<Option<Value>, ApiError>;
    async fn obtener_calidad(&self, horas: i32) -> Result<Value, ApiError>;
    async fn registrar_venta(
        &self,
        data: &VentaGeolocalizada,
        evento_id: Option<&str>,
    ) -> Result<(), ApiError>;
}

#[async_trait]
impl InteligenciaPort for Inteligencia {
    async fn obtener_mapa_calor(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        Inteligencia::obtener_mapa_calor(self, filtros).await
    }
    async fn obtener_rendimiento(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        Inteligencia::obtener_rendimiento(self, filtros).await
    }
    async fn obtener_cobertura(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        Inteligencia::obtener_cobertura(self, filtros).await
    }
    async fn obtener_demanda(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        Inteligencia::obtener_demanda(self, filtros).await
    }
    async fn obtener_resumen(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        Inteligencia::obtener_resumen(self, filtros).await
    }
    async fn obtener_tendencias(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        Inteligencia::obtener_tendencias(self, filtros).await
    }
    async fn obtener_forecast(&self, sku: Option<&str>, dias: i32) -> Result<Value, ApiError> {
        Inteligencia::obtener_forecast(self, sku, dias).await
    }
    async fn obtener_anomalias(&self, filtros: &Filtros) -> Result<Value, ApiError> {
        Inteligencia::obtener_anomalias(self, filtros).await
    }
    async fn obtener_score(&self, vendedor_id: &str) -> Result<Option<Value>, ApiError> {
        Inteligencia::obtener_score(self, vendedor_id).await
    }
    async fn obtener_calidad(&self, horas: i32) -> Result<Value, ApiError> {
        Inteligencia::obtener_calidad(self, horas).await
    }
    async fn registrar_venta(
        &self,
        data: &VentaGeolocalizada,
        evento_id: Option<&str>,
    ) -> Result<(), ApiError> {
        Inteligencia::procesar_venta_geolocalizada(self, data, evento_id).await
    }
}

pub struct InteligenciaNoDisponible;

#[async_trait]
impl InteligenciaPort for InteligenciaNoDisponible {
    async fn obtener_mapa_calor(&self, _: &Filtros) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_rendimiento(&self, _: &Filtros) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_cobertura(&self, _: &Filtros) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_demanda(&self, _: &Filtros) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_resumen(&self, _: &Filtros) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_tendencias(&self, _: &Filtros) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_forecast(&self, _: Option<&str>, _: i32) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_anomalias(&self, _: &Filtros) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_score(&self, _: &str) -> Result<Option<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn obtener_calidad(&self, _: i32) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn registrar_venta(
        &self,
        _: &VentaGeolocalizada,
        _: Option<&str>,
    ) -> Result<(), ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
}

// ── Helpers compartidos por consultas e ingesta ───────────────────────────

pub fn error_db(error: sqlx::Error) -> ApiError {
    logs::error(
        "inteligencia",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

pub fn parsear_uuid(valor: &str) -> Result<uuid::Uuid, ApiError> {
    uuid::Uuid::parse_str(valor).map_err(|_| ApiError::interno())
}

/// `parseInt` de JavaScript: acepta un prefijo numérico y corta en el resto.
pub fn parse_int_js(valor: &str) -> Option<i32> {
    let texto = valor.trim();
    let mut fin = 0;
    let bytes = texto.as_bytes();
    if fin < bytes.len() && (bytes[fin] == b'-' || bytes[fin] == b'+') {
        fin += 1;
    }
    let inicio_digitos = fin;
    while fin < bytes.len() && bytes[fin].is_ascii_digit() {
        fin += 1;
    }
    if fin == inicio_digitos {
        return None;
    }
    texto[..fin].parse::<i32>().ok()
}

/// `parseFloat` de JavaScript con degradación a 0 (NaN no es serializable).
pub fn parse_float_js(valor: &str) -> f64 {
    let texto = valor.trim();
    let mut fin = 0;
    let bytes = texto.as_bytes();
    if fin < bytes.len() && (bytes[fin] == b'-' || bytes[fin] == b'+') {
        fin += 1;
    }
    let mut digito = false;
    while fin < bytes.len() && bytes[fin].is_ascii_digit() {
        digito = true;
        fin += 1;
    }
    if fin < bytes.len() && bytes[fin] == b'.' {
        fin += 1;
        while fin < bytes.len() && bytes[fin].is_ascii_digit() {
            digito = true;
            fin += 1;
        }
    }
    if !digito {
        return 0.0;
    }
    texto[..fin].parse::<f64>().unwrap_or(0.0)
}

fn cursor_par(cursor: &str) -> Option<(String, String)> {
    let mut partes = cursor.split('_');
    let primero = partes.next()?.to_string();
    let segundo = partes.next()?.to_string();
    Some((primero, segundo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_replicar_parse_int_de_javascript() {
        assert_eq!(parse_int_js("500"), Some(500));
        assert_eq!(parse_int_js("12abc"), Some(12));
        assert_eq!(parse_int_js(" -7.9"), Some(-7));
        assert_eq!(parse_int_js("abc"), None);
        assert_eq!(parse_int_js(""), None);
    }

    #[test]
    fn debe_replicar_parse_float_de_javascript() {
        assert_eq!(parse_float_js("12.5"), 12.5);
        assert_eq!(parse_float_js("1.5abc"), 1.5);
        assert_eq!(parse_float_js("abc"), 0.0);
        assert_eq!(parse_float_js(""), 0.0);
    }

    #[test]
    fn debe_partir_el_cursor_en_dos_partes_como_javascript() {
        assert_eq!(
            cursor_par("42_00000000-0000-4000-8000-000000000001"),
            Some((
                "42".to_string(),
                "00000000-0000-4000-8000-000000000001".to_string()
            ))
        );
        assert_eq!(cursor_par("sin-separador"), None);
        assert_eq!(
            cursor_par("1_SKU_con_guiones"),
            Some(("1".to_string(), "SKU".to_string()))
        );
    }

    #[test]
    fn debe_mapear_un_uuid_invalido_a_500() {
        assert_eq!(
            parsear_uuid("no-es-uuid").expect_err("500").status,
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
