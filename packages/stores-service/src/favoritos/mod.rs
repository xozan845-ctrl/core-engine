//! Wishlist del comprador (favoritos por item).

use crate::clients::Catalogo;
use crate::db::Db;
use crate::errors::ApiError;
use serde_json::Value;
use std::sync::Arc;

pub struct Favoritos {
    db: Arc<Db>,
    catalogo: Arc<dyn Catalogo>,
}

impl Favoritos {
    pub fn nueva(db: Arc<Db>, catalogo: Arc<dyn Catalogo>) -> Self {
        Self { db, catalogo }
    }

    pub async fn listar(&self, comprador_id: &str) -> Result<Vec<Value>, ApiError> {
        let filas = sqlx::query(
            "SELECT f.item_id, f.empresa_id, f.creado_en
               FROM stores.favoritos f
              WHERE f.comprador_id = $1::uuid
              ORDER BY f.creado_en DESC LIMIT 200",
        )
        .bind(comprador_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        filas
            .iter()
            .map(|fila| crate::db::fila_a_json(fila).map_err(crate::db::interno))
            .collect()
    }

    pub async fn agregar(&self, comprador_id: &str, item_id: &str) -> Result<(), ApiError> {
        let item = self
            .catalogo
            .item(item_id)
            .await
            .ok_or_else(|| ApiError::no_encontrado("Item", item_id))?;
        sqlx::query(
            "INSERT INTO stores.favoritos (comprador_id, item_id, empresa_id)
             VALUES ($1::uuid, $2::uuid, $3::uuid)
             ON CONFLICT (comprador_id, item_id) DO NOTHING",
        )
        .bind(comprador_id)
        .bind(item_id)
        .bind(&item.empresa_id)
        .execute(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        Ok(())
    }

    pub async fn eliminar(&self, comprador_id: &str, item_id: &str) -> Result<(), ApiError> {
        sqlx::query(
            "DELETE FROM stores.favoritos
              WHERE comprador_id = $1::uuid AND item_id = $2::uuid",
        )
        .bind(comprador_id)
        .bind(item_id)
        .execute(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        Ok(())
    }
}
