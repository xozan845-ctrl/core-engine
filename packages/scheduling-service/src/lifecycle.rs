//! Ciclo de vida del tenant en agenda y citas.

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
    for (nombre, tabla) in [
        ("recursos", "scheduling.recursos"),
        ("disponibilidad", "scheduling.disponibilidad"),
        ("bloqueos", "scheduling.bloqueos"),
        ("citas", "scheduling.citas"),
        ("recordatorios", "scheduling.recordatorios"),
    ] {
        let valor: Value = sqlx::query_scalar(&format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM {tabla} t WHERE t.empresa_id = $1"
        ))
        .bind(empresa)
        .fetch_one(pool)
        .await?;
        datos.insert(nombre.into(), valor);
    }
    Ok(Value::Object(datos))
}

pub async fn purgar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let empresa = empresa(req).map_err(sqlx::Error::Protocol)?;
    let mut tx = pool.begin().await?;
    for tabla in [
        "scheduling.recordatorios",
        "scheduling.citas",
        "scheduling.bloqueos",
        "scheduling.disponibilidad",
        "scheduling.recursos",
    ] {
        sqlx::query(&format!("DELETE FROM {tabla} WHERE empresa_id = $1"))
            .bind(empresa)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
