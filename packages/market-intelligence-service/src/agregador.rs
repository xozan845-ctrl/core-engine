//! Agregación atómica de ventas, comisiones y stock en el esquema
//! `intelligence` (upserts idempotentes, R-DI-3).

use crate::config::EMPRESA_MERCADO_ID;
use crate::errors::ApiError;
use crate::inteligencia::{error_db, parsear_uuid, Inteligencia, VentaGeolocalizada};
use crate::logs;
use async_trait::async_trait;
use chrono::{Datelike, Utc};
use serde_json::json;

#[async_trait]
pub trait AgregadorPort: Send + Sync {
    async fn procesar_venta_geolocalizada(
        &self,
        data: &VentaGeolocalizada,
        evento_id: Option<&str>,
    ) -> Result<(), ApiError>;

    async fn procesar_orden_completada(
        &self,
        order_id: &str,
        vendedor_id: &str,
    ) -> Result<(), ApiError>;

    async fn procesar_stock_actualizado(
        &self,
        empresa_id: &str,
        sku: &str,
        stock_restante: i32,
        nombre: Option<&str>,
    ) -> Result<(), ApiError>;
}

impl Inteligencia {
    pub async fn procesar_venta_geolocalizada(
        &self,
        data: &VentaGeolocalizada,
        evento_id: Option<&str>,
    ) -> Result<(), ApiError> {
        if data.skus.is_empty() {
            return Err(ApiError::nueva(
                "VENTA_SIN_SKUS",
                "La venta no tiene SKUs.",
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }

        let order_uuid = parsear_uuid(&data.order_id)?;
        let vendedor_uuid = parsear_uuid(&data.vendedor_id)?;
        let evento_uuid = match evento_id {
            Some(valor) => Some(parsear_uuid(valor)?),
            None => None,
        };
        let empresa_mercado = parsear_uuid(EMPRESA_MERCADO_ID)?;

        let mut tx = self.db.transaccion().await.map_err(error_db)?;

        // Idempotencia atómica: la marca y todas las vistas comparten transacción.
        let marca = sqlx::query(
            "INSERT INTO intelligence.eventos_procesados (event_id, tipo, procesado_en)
             VALUES (COALESCE($1::uuid, md5($2)::uuid), 'venta.geolocalizada', NOW())
             ON CONFLICT (event_id) DO NOTHING
             RETURNING event_id",
        )
        .bind(evento_uuid)
        .bind(&data.order_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if marca.is_none() {
            tx.commit().await.map_err(error_db)?;
            logs::warn(
                "inteligencia.agregador",
                crate::campos! {
                    "msg": "Venta geolocalizada duplicada ignorada",
                    "order_id": data.order_id,
                    "evento_id": evento_id,
                },
            );
            return Ok(());
        }

        let cantidad_skus = data.skus.len() as i64;
        let monto_por_sku = data.monto_cents / cantidad_skus;
        let ocurrido_en = Utc::now();

        // 1. Hechos atómicos (idempotencia por order_id + sku).
        for sku in &data.skus {
            sqlx::query(
                "INSERT INTO intelligence.hechos_venta
                  (order_id, sku, vendedor_id, cantidad, monto_cents, lat, lng,
                   gps_precision_metros, gps_velocidad_ms, gps_rumbo_grados,
                   tipo_actividad, resultado_visita, distancia_cliente_metros,
                   rango_edad, genero, ocurrido_en)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                 ON CONFLICT (order_id, sku) DO NOTHING",
            )
            .bind(order_uuid)
            .bind(sku)
            .bind(vendedor_uuid)
            .bind(1_i32)
            .bind(monto_por_sku)
            .bind(data.lat)
            .bind(data.lng)
            .bind(data.precision)
            .bind(data.velocidad)
            .bind(data.rumbo)
            .bind(data.tipo_actividad.as_deref())
            .bind(data.resultado_visita.as_deref())
            .bind(data.distancia_cliente_metros)
            .bind(data.rango_edad.as_deref())
            .bind(data.genero.as_deref())
            .bind(ocurrido_en)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }

        // 2. Punto de calor (alta resolución, uno por venta).
        if let (Some(lat), Some(lng)) = (data.lat, data.lng) {
            sqlx::query(
                "INSERT INTO intelligence.puntos_calor
                   (lat, lng, geom, peso, vendedor_id, tipo, ocurrido_en)
                 VALUES ($1, $2, ST_SetSRID(ST_MakePoint($2, $1), 4326), $3, $4, 'venta', NOW())",
            )
            .bind(lat)
            .bind(lng)
            .bind(1_i32)
            .bind(vendedor_uuid)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }

        // 3. Rendimiento del vendedor.
        sqlx::query(
            "INSERT INTO intelligence.rendimiento_vendedor
               (vendedor_id, total_ventas, total_monto_cents, ticket_promedio_cents, actualizado_en)
             VALUES ($1, 1, $2, $2, NOW())
             ON CONFLICT (vendedor_id) DO UPDATE SET
               total_ventas          = rendimiento_vendedor.total_ventas + 1,
               total_monto_cents     = rendimiento_vendedor.total_monto_cents + EXCLUDED.total_monto_cents,
               ticket_promedio_cents = (rendimiento_vendedor.total_monto_cents + EXCLUDED.total_monto_cents)
                                        / (rendimiento_vendedor.total_ventas + 1),
               actualizado_en        = NOW()",
        )
        .bind(vendedor_uuid)
        .bind(data.monto_cents)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;

        // 4. Cobertura por celda de ~1.1 km (0.01 grados).
        if let (Some(lat), Some(lng)) = (data.lat, data.lng) {
            let lat_cell = (lat * 100.0).floor() / 100.0;
            let lng_cell = (lng * 100.0).floor() / 100.0;
            sqlx::query(
                "INSERT INTO intelligence.cobertura_zona
                   (lat_cell, lng_cell, total_ventas, total_monto_cents, total_vendedores, actualizado_en)
                 VALUES ($1, $2, 1, $3, 1, NOW())
                 ON CONFLICT (lat_cell, lng_cell) DO UPDATE SET
                   total_ventas      = cobertura_zona.total_ventas + 1,
                   total_monto_cents = cobertura_zona.total_monto_cents + EXCLUDED.total_monto_cents,
                   actualizado_en    = NOW()",
            )
            .bind(lat_cell)
            .bind(lng_cell)
            .bind(data.monto_cents)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }

        // 5. Métricas por producto en el bucket de mercado (ADR-27).
        for sku in &data.skus {
            sqlx::query(
                "INSERT INTO intelligence.metricas_producto
                   (empresa_id, sku, total_vendido, total_monto_cents, ultima_venta_en, actualizado_en)
                 VALUES ($1, $2, 1, $3, NOW(), NOW())
                 ON CONFLICT (empresa_id, sku) DO UPDATE SET
                   total_vendido     = metricas_producto.total_vendido + 1,
                   total_monto_cents = metricas_producto.total_monto_cents + EXCLUDED.total_monto_cents,
                   ultima_venta_en   = NOW(),
                   actualizado_en    = NOW()",
            )
            .bind(empresa_mercado)
            .bind(sku)
            .bind(monto_por_sku)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }

        // 6. Tendencias temporales por día, SKU y vendedor.
        let fecha = Utc::now().format("%Y-%m-%d").to_string();
        let dia_semana = chrono::Local::now().weekday().num_days_from_sunday() as i32;
        for sku in &data.skus {
            sqlx::query(
                "INSERT INTO intelligence.tendencias_temporales
                   (fecha, sku, vendedor_id, total_ventas, total_monto_cents, dia_semana)
                 VALUES ($1::date, $2, $3, 1, $4, $5)
                 ON CONFLICT (fecha, sku, vendedor_id) DO UPDATE SET
                   total_ventas      = tendencias_temporales.total_ventas + 1,
                   total_monto_cents = tendencias_temporales.total_monto_cents + EXCLUDED.total_monto_cents",
            )
            .bind(&fecha)
            .bind(sku)
            .bind(vendedor_uuid)
            .bind(monto_por_sku)
            .bind(dia_semana)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }

        tx.commit().await.map_err(error_db)?;

        let geo = match (data.lat, data.lng) {
            (Some(lat), Some(lng)) => json!({ "lat": lat, "lng": lng }),
            _ => serde_json::Value::Null,
        };
        logs::info(
            "inteligencia.agregador",
            crate::campos! {
                "msg": "Venta geolocalizada agregada",
                "order_id": data.order_id,
                "vendedor_id": data.vendedor_id,
                "skus_count": data.skus.len(),
                "monto_cents": data.monto_cents,
                "geo": geo,
            },
        );
        Ok(())
    }

    pub async fn procesar_orden_completada(
        &self,
        order_id: &str,
        vendedor_id: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE intelligence.rendimiento_vendedor
             SET pedidos_entregados = pedidos_entregados + 1,
                 tasa_efectividad   = CASE
                   WHEN (pedidos_entregados + pedidos_fallidos + 1) > 0
                   THEN ROUND(((pedidos_entregados + 1)::numeric /
                               (pedidos_entregados + pedidos_fallidos + 1)) * 100, 2)
                   ELSE 0 END,
                 actualizado_en     = NOW()
             WHERE vendedor_id = $1::uuid",
        )
        .bind(parsear_uuid(vendedor_id)?)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        logs::info(
            "inteligencia.agregador",
            crate::campos! { "msg": "Orden completada registrada", "order_id": order_id },
        );
        Ok(())
    }

    pub async fn procesar_stock_actualizado(
        &self,
        empresa_id: &str,
        sku: &str,
        stock_restante: i32,
        nombre: Option<&str>,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO intelligence.metricas_producto
               (empresa_id, sku, nombre, stock_actual, actualizado_en)
             VALUES ($1::uuid, $2, $3, $4, NOW())
             ON CONFLICT (empresa_id, sku) DO UPDATE SET
               stock_actual   = EXCLUDED.stock_actual,
               nombre         = COALESCE(EXCLUDED.nombre, metricas_producto.nombre),
               actualizado_en = NOW()",
        )
        .bind(parsear_uuid(empresa_id)?)
        .bind(sku)
        .bind(nombre.unwrap_or(""))
        .bind(stock_restante)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(())
    }
}

#[async_trait]
impl AgregadorPort for Inteligencia {
    async fn procesar_venta_geolocalizada(
        &self,
        data: &VentaGeolocalizada,
        evento_id: Option<&str>,
    ) -> Result<(), ApiError> {
        Inteligencia::procesar_venta_geolocalizada(self, data, evento_id).await
    }

    async fn procesar_orden_completada(
        &self,
        order_id: &str,
        vendedor_id: &str,
    ) -> Result<(), ApiError> {
        Inteligencia::procesar_orden_completada(self, order_id, vendedor_id).await
    }

    async fn procesar_stock_actualizado(
        &self,
        empresa_id: &str,
        sku: &str,
        stock_restante: i32,
        nombre: Option<&str>,
    ) -> Result<(), ApiError> {
        Inteligencia::procesar_stock_actualizado(self, empresa_id, sku, stock_restante, nombre)
            .await
    }
}

pub struct AgregadorNoDisponible;

#[async_trait]
impl AgregadorPort for AgregadorNoDisponible {
    async fn procesar_venta_geolocalizada(
        &self,
        _: &VentaGeolocalizada,
        _: Option<&str>,
    ) -> Result<(), ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn procesar_orden_completada(&self, _: &str, _: &str) -> Result<(), ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }

    async fn procesar_stock_actualizado(
        &self,
        _: &str,
        _: &str,
        _: i32,
        _: Option<&str>,
    ) -> Result<(), ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn debe_fallar_cerrado_sin_postgres() {
        let agregador = AgregadorNoDisponible;
        let data = VentaGeolocalizada {
            order_id: "o-1".into(),
            vendedor_id: "v-1".into(),
            skus: vec!["SKU-1".into()],
            monto_cents: 100,
            lat: None,
            lng: None,
            precision: None,
            velocidad: None,
            rumbo: None,
            tipo_actividad: None,
            resultado_visita: None,
            distancia_cliente_metros: None,
            rango_edad: None,
            genero: None,
        };
        let error = agregador
            .procesar_venta_geolocalizada(&data, None)
            .await
            .expect_err("503");
        assert_eq!(error.codigo, "SERVICIO_NO_DISPONIBLE");
    }
}
