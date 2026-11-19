//! Ciclo de vida del tenant en inventario: exportación y purga.

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
        ("configuracion", "inventory.configuracion_inventario"),
        ("proveedores", "inventory.proveedores"),
        ("ubicaciones", "inventory.ubicaciones"),
        ("stock_ubicacion", "inventory.item_stock_ubicacion"),
        ("lotes", "inventory.lotes_inventario"),
        ("movimientos", "inventory.movimientos_inventario"),
        ("ajustes", "inventory.ajustes_stock"),
        ("reservas_lotes", "inventory.reservas_lotes"),
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
        "inventory.reservas_lotes",
        "inventory.item_stock_ubicacion",
        "inventory.movimientos_inventario",
        "inventory.ajustes_stock",
        "inventory.lotes_inventario",
        "inventory.ubicaciones",
        "inventory.proveedores",
        "inventory.configuracion_inventario",
    ] {
        sqlx::query(&format!("DELETE FROM {tabla} WHERE empresa_id = $1"))
            .bind(empresa)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "DELETE FROM inventory.reservas_ordenes
          WHERE order_id IN (
              SELECT id FROM orders.orden_vista
               WHERE vendedor_id = ANY($1::uuid[]) OR negocio_id = ANY($2::uuid[])
          )",
    )
    .bind(
        req.usuario_ids
            .iter()
            .filter_map(|valor| Uuid::parse_str(valor).ok())
            .collect::<Vec<_>>(),
    )
    .bind(
        req.negocio_ids
            .iter()
            .filter_map(|valor| Uuid::parse_str(valor).ok())
            .collect::<Vec<_>>(),
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(json!({ "empresa_id": req.empresa_id, "purgado": true }))
}
