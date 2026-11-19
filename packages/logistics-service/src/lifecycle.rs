//! Ciclo de vida del tenant en logística.

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

pub async fn exportar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let empresa = empresa(req).map_err(sqlx::Error::Protocol)?;
    let mut datos = Map::new();
    let envios: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM logistics.envios t WHERE t.empresa_id = $1",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("envios".into(), envios);
    let asignaciones: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM logistics.asignaciones_envio t
          WHERE t.envio_id IN (SELECT id FROM logistics.envios WHERE empresa_id = $1)",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("asignaciones".into(), asignaciones);
    Ok(Value::Object(datos))
}

pub async fn purgar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let empresa = empresa(req).map_err(sqlx::Error::Protocol)?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM logistics.asignaciones_envio
          WHERE envio_id IN (SELECT id FROM logistics.envios WHERE empresa_id = $1)",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM logistics.envios WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
