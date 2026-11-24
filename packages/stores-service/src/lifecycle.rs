//! Ciclo de vida del tenant en tiendas y ofertas.

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
    let tiendas: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM stores.tiendas t WHERE t.empresa_id = $1",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("tiendas".into(), tiendas);
    for (nombre, sql) in [
        (
            "ofertas",
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM stores.ofertas t
              WHERE t.tienda_id IN (SELECT id FROM stores.tiendas WHERE empresa_id = $1)",
        ),
        (
            "historico_precios",
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM stores.historico_precios t
              WHERE t.oferta_id IN (
                  SELECT o.id FROM stores.ofertas o
                    JOIN stores.tiendas ti ON ti.id = o.tienda_id
                   WHERE ti.empresa_id = $1
              )",
        ),
        (
            "slugs",
            "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
               FROM stores.tienda_slugs t
              WHERE t.tienda_id IN (SELECT id FROM stores.tiendas WHERE empresa_id = $1)",
        ),
    ] {
        let valor: Value = sqlx::query_scalar(sql)
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
    sqlx::query(
        "DELETE FROM stores.historico_precios
          WHERE oferta_id IN (
              SELECT o.id FROM stores.ofertas o
                JOIN stores.tiendas ti ON ti.id = o.tienda_id
               WHERE ti.empresa_id = $1
          )",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM stores.ofertas
          WHERE tienda_id IN (SELECT id FROM stores.tiendas WHERE empresa_id = $1)",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM stores.tienda_slugs
          WHERE tienda_id IN (SELECT id FROM stores.tiendas WHERE empresa_id = $1)",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM stores.tiendas WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
