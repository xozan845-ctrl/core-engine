//! Ciclo de vida del tenant en comisiones.

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
    let usuarios = usuarios(req);
    let negocios: Vec<Uuid> = req
        .negocio_ids
        .iter()
        .filter_map(|valor| Uuid::parse_str(valor).ok())
        .collect();
    let mut datos = Map::new();
    let comisiones: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM commissions.comisiones t WHERE t.empresa_id = $1",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("comisiones".into(), comisiones);
    let compensaciones: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM commissions.compensaciones_devoluciones t WHERE t.empresa_id = $1",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("compensaciones".into(), compensaciones);
    let liquidaciones: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM commissions.liquidaciones t WHERE t.vendedor_id = ANY($1::uuid[])",
    )
    .bind(&usuarios)
    .fetch_one(pool)
    .await?;
    datos.insert("liquidaciones".into(), liquidaciones);
    let liquidaciones_empresas: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM commissions.liquidaciones_empresas t WHERE t.empresa_id = $1",
    )
    .bind(empresa)
    .fetch_one(pool)
    .await?;
    datos.insert("liquidaciones_empresas".into(), liquidaciones_empresas);
    let pagos: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(t)), '[]'::jsonb)
           FROM commissions.pagos t
          WHERE t.order_id IN (
              SELECT id FROM orders.orden_vista
               WHERE negocio_id = ANY($1::uuid[]) OR vendedor_id = ANY($2::uuid[])
          )",
    )
    .bind(&negocios)
    .bind(&usuarios)
    .fetch_one(pool)
    .await?;
    datos.insert("pagos".into(), pagos);
    Ok(Value::Object(datos))
}

pub async fn purgar(pool: &PgPool, req: &TenantRequest) -> Result<Value, sqlx::Error> {
    let empresa = empresa(req).map_err(sqlx::Error::Protocol)?;
    let usuarios = usuarios(req);
    let negocios: Vec<Uuid> = req
        .negocio_ids
        .iter()
        .filter_map(|valor| Uuid::parse_str(valor).ok())
        .collect();
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM commissions.compensaciones_devoluciones WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM commissions.comisiones WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM commissions.liquidaciones WHERE vendedor_id = ANY($1::uuid[])")
        .bind(&usuarios)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM commissions.liquidaciones_empresas WHERE empresa_id = $1")
        .bind(empresa)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "DELETE FROM commissions.pagos
          WHERE order_id IN (
              SELECT id FROM orders.orden_vista
               WHERE negocio_id = ANY($1::uuid[]) OR vendedor_id = ANY($2::uuid[])
          )",
    )
    .bind(&negocios)
    .bind(&usuarios)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
