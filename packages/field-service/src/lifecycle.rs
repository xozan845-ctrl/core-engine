//! Ciclo de vida del tenant en operaciones de campo.

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

const TABLAS: [(&str, &str); 11] = [
    ("asistencia", "field.asistencia"),
    ("clientes", "field.clientes"),
    ("cumplimiento", "field.cumplimiento"),
    ("incidencias", "field.incidencias"),
    ("paradas", "field.paradas"),
    ("pedidos", "field.pedidos"),
    ("personal", "field.personal"),
    ("rutas", "field.rutas"),
    ("tracking", "field.tracking"),
    ("vehiculos", "field.vehiculos"),
    ("visitas", "field.visitas"),
];

pub async fn exportar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let empresa = empresa(req).map_err(sqlx::Error::Protocol)?;
    let mut datos = Map::new();
    for (nombre, tabla) in TABLAS {
        let valor: Value = sqlx::query_scalar(&format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM {tabla} t WHERE t.tenant_id = $1"
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
    for (_, tabla) in [
        ("tracking", "field.tracking"),
        ("paradas", "field.paradas"),
        ("incidencias", "field.incidencias"),
        ("visitas", "field.visitas"),
        ("asistencia", "field.asistencia"),
        ("cumplimiento", "field.cumplimiento"),
        ("pedidos", "field.pedidos"),
        ("rutas", "field.rutas"),
        ("clientes", "field.clientes"),
        ("personal", "field.personal"),
        ("vehiculos", "field.vehiculos"),
    ] {
        sqlx::query(&format!("DELETE FROM {tabla} WHERE tenant_id = $1"))
            .bind(empresa)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
