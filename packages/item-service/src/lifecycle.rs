//! Ciclo de vida del tenant en el catálogo maestro (`item`).

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
    let items: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM item.items t WHERE t.empresa_id = $1",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("items".into(), items);
    for (nombre, tabla) in [
        ("atributo_esquemas", "item.item_atributo_esquemas"),
        ("imagenes", "item.item_imagenes"),
        ("series", "item.item_series"),
        ("componentes", "item.item_componentes"),
    ] {
        let valor: Value = sqlx::query_scalar(&format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM {tabla} t
              WHERE t.empresa_id = $1
                 OR t.item_id IN (SELECT id FROM item.items WHERE empresa_id = $1)"
        ))
        .bind(empresa)
        .fetch_one(pool)
        .await?;
        datos.insert(nombre.into(), valor);
    }
    let historico: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM item.historico_precios t WHERE t.empresa_id = $1",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("historico_precios".into(), historico);
    Ok(Value::Object(datos))
}

pub async fn purgar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let empresa = empresa(req).map_err(sqlx::Error::Protocol)?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        "DELETE FROM item.item_componentes
          WHERE item_id IN (SELECT id FROM item.items WHERE empresa_id = $1)
             OR componente_item_id IN (SELECT id FROM item.items WHERE empresa_id = $1)",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await?;
    for tabla in ["item.item_imagenes", "item.item_series"] {
        sqlx::query(&format!(
            "DELETE FROM {tabla}
              WHERE item_id IN (SELECT id FROM item.items WHERE empresa_id = $1)"
        ))
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("DELETE FROM item.item_atributo_esquemas WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM item.historico_precios WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM item.items WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
