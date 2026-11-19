//! Ciclo de vida del tenant en inteligencia de mercado.
//!
//! `cobertura_zona` es un agregado geográfico sin tenant; se conserva.

use serde::Deserialize;
use serde_json::{json, Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct TenantRequest {
    pub empresa_id: String,
    #[serde(default)]
    pub negocio_ids: Vec<String>,
    #[serde(default)]
    pub usuario_ids: Vec<String>,
}

fn empresa(req: &TenantRequest) -> Result<Uuid, String> {
    Uuid::parse_str(&req.empresa_id).map_err(|_| "empresa_id debe ser UUID".to_string())
}

fn usuarios(req: &TenantRequest) -> Vec<Uuid> {
    req.usuario_ids
        .iter()
        .filter_map(|valor| Uuid::parse_str(valor).ok())
        .collect()
}

pub async fn exportar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let empresa = empresa(req).map_err(sqlx::Error::Protocol)?;
    let mut datos = Map::new();
    let metricas: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM intelligence.metricas_producto t
          WHERE t.sku IN (SELECT codigo FROM item.items WHERE empresa_id = $1)",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("metricas_producto".into(), metricas);
    for (nombre, tabla) in [
        ("hechos_venta", "intelligence.hechos_venta"),
        ("puntos_calor", "intelligence.puntos_calor"),
        ("rendimiento_vendedor", "intelligence.rendimiento_vendedor"),
        (
            "tendencias_temporales",
            "intelligence.tendencias_temporales",
        ),
    ] {
        let valor: Value = sqlx::query_scalar(&format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM {tabla} t WHERE t.vendedor_id = ANY($1::uuid[])"
        ))
        .bind(usuarios(req))
        .fetch_one(pool)
        .await?;
        datos.insert(nombre.into(), valor);
    }
    Ok(Value::Object(datos))
}

pub async fn purgar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let usuarios = usuarios(req);
    let mut tx = pool.begin().await?;
    // `metricas_producto` es un agregado global por SKU (sin PII); no se borra
    // para no afectar a otros tenants que vendan el mismo SKU.
    for tabla in [
        "intelligence.hechos_venta",
        "intelligence.puntos_calor",
        "intelligence.rendimiento_vendedor",
        "intelligence.tendencias_temporales",
    ] {
        sqlx::query(&format!(
            "DELETE FROM {tabla} WHERE vendedor_id = ANY($1::uuid[])"
        ))
        .bind(&usuarios)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
