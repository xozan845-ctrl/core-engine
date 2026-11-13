#![allow(dead_code)]

//! Utilidades de integración: esquema `catalog` y fixtures.

use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

pub use catalog_service::db::Db;

pub const EMPRESA: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
pub const EMPRESA_2: &str = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
pub const USUARIO: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

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

pub fn payload_item(codigo: &str, precio: &str, stock: i64) -> serde_json::Value {
    json!({
        "tipo": "producto",
        "codigo": codigo,
        "nombre": "Producto de prueba",
        "precio": precio,
        "stock": stock,
        "atributos": {}
    })
}
