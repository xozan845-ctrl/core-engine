#![allow(dead_code)]

//! Utilidades de integración: esquema `catalog` transitorio y fixtures.

use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

pub use inventory_service::db::Db;

pub const EMPRESA: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
pub const ITEM: &str = "11111111-1111-4111-8111-111111111111";

pub fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

pub async fn provisionar(url: &str) {
    let pool = PgPool::connect(url).await.expect("postgres");
    sqlx::raw_sql(include_str!("../schema.sql"))
        .execute(&pool)
        .await
        .expect("esquema catalog");
    pool.close().await;
}

pub async fn db() -> Arc<Db> {
    Arc::new(
        Db::conectar(&url().expect("TEST_DATABASE_URL"))
            .await
            .expect("db"),
    )
}

pub async fn crear_item(db: &Db, id: &str, codigo: &str, stock: i32, precio: i32) {
    sqlx::query(
        "INSERT INTO catalog.items
           (id, empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock, estado)
         VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, true, $6, 'activo')",
    )
    .bind(id)
    .bind(EMPRESA)
    .bind(codigo)
    .bind(format!("Item {codigo}"))
    .bind(precio)
    .bind(stock)
    .execute(db.pool())
    .await
    .expect("item");
}

pub fn item_json() -> serde_json::Value {
    json!({
        "empresa_id": EMPRESA,
        "item_id": ITEM,
        "track_inventory": true,
        "stock": 15,
        "precio_cents": 1000
    })
}
