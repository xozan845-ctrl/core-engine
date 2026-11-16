//! Tipos de orden y transiciones configurables (Fase 3).
//!
//! La matriz vive en `orders.tipos_orden`/`orders.transiciones_orden`. El
//! código la consulta en cada transición, así un admin puede agregar tipos o
//! aristas sin desplegar.

use crate::db::Db;
use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TipoOrden {
    pub codigo: String,
    pub nombre: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransicionOrden {
    pub desde: String,
    pub hasta: String,
}

pub struct TiposOrden {
    db: Arc<Db>,
}

impl TiposOrden {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(&self) -> Result<Vec<TipoOrden>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, activo, config
               FROM orders.tipos_orden ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_tipo).collect()
    }

    pub async fn obtener(&self, codigo: &str) -> Result<Option<TipoOrden>, ApiError> {
        let codigo = codigo.trim().to_lowercase();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT codigo, nombre, activo, config
               FROM orders.tipos_orden WHERE codigo = $1",
        )
        .bind(&codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_tipo).transpose()
    }

    pub async fn transiciones(&self, tipo: &str) -> Result<Vec<TransicionOrden>, ApiError> {
        let tipo = tipo.trim().to_lowercase();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT desde, hasta FROM orders.transiciones_orden
              WHERE tipo_codigo = $1 ORDER BY desde, hasta",
        )
        .bind(&tipo)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas
            .iter()
            .map(|fila| {
                Ok(TransicionOrden {
                    desde: fila
                        .try_get("desde")
                        .map_err(crate::event_store::error_db)?,
                    hasta: fila
                        .try_get("hasta")
                        .map_err(crate::event_store::error_db)?,
                })
            })
            .collect()
    }

    /// Normaliza y exige que el tipo exista y esté activo.
    pub async fn validar_tipo(&self, codigo: &str) -> Result<String, ApiError> {
        let codigo = codigo.trim().to_lowercase();
        let tipo = self.obtener(&codigo).await?;
        match tipo {
            Some(tipo) if tipo.activo => Ok(tipo.codigo),
            _ => Err(ApiError::dominio(
                "TIPO_ORDEN_INVALIDO",
                "El tipo de orden no es valido.",
            )),
        }
    }

    /// Valida una transición contra la matriz configurada.
    pub async fn validar(&self, tipo: &str, desde: &str, hasta: &str) -> Result<(), ApiError> {
        let tipo = self.validar_tipo(tipo).await?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let existe = sqlx::query(
            "SELECT 1 FROM orders.transiciones_orden
              WHERE tipo_codigo = $1 AND desde = $2 AND hasta = $3",
        )
        .bind(&tipo)
        .bind(desde)
        .bind(hasta)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        if existe.is_some() {
            return Ok(());
        }
        Err(ApiError::dominio(
            "TRANSICION_INVALIDA",
            format!(
                "Transicion invalida de estado \"{desde}\" a \"{hasta}\" para orden \"{tipo}\"."
            ),
        ))
    }
}

fn fila_tipo(fila: &sqlx::postgres::PgRow) -> Result<TipoOrden, ApiError> {
    Ok(TipoOrden {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}
