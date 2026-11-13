//! Entrega digital (Fase P2.1): acceso por línea con token, límites de
//! descarga y expiración. Réplica de `entregas-digitales.service.ts`.

use crate::auth::UsuarioContexto;
use crate::db::iso_ms;
use crate::errors::ApiError;
use crate::event_store::error_db;
use serde::Serialize;
use sqlx::{postgres::PgRow, Row};

const ESTADOS_PAGADOS: [&str; 3] = ["pagada", "enviada", "entregada"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EntregaDigital {
    pub id: String,
    pub order_id: String,
    pub item_id: String,
    pub sku: String,
    pub token: String,
    pub estado: String,
    pub descargas: i32,
    pub max_descargas: Option<i32>,
    pub expira_en: Option<String>,
    pub primera_descarga_en: Option<String>,
    pub ultima_descarga_en: Option<String>,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DescargaDigital {
    pub id: String,
    pub order_id: String,
    pub item_id: String,
    pub sku: String,
    pub token: String,
    pub estado: String,
    pub descargas: i32,
    pub max_descargas: Option<i32>,
    pub expira_en: Option<String>,
    pub primera_descarga_en: Option<String>,
    pub ultima_descarga_en: Option<String>,
    pub creado_en: String,
    pub recurso_url: String,
    pub comprador_id: String,
}

pub struct EntregasDigitales {
    db: std::sync::Arc<crate::db::Db>,
}

impl EntregasDigitales {
    pub fn nueva(db: std::sync::Arc<crate::db::Db>) -> Self {
        Self { db }
    }

    pub async fn listar_por_orden(
        &self,
        order_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<EntregaDigital>, ApiError> {
        let order_uuid = validar_uuid(order_id, "orden")?;
        let orden = sqlx::query("SELECT comprador_id FROM orders.orden_vista WHERE id = $1")
            .bind(order_uuid)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
        let Some(orden) = orden else {
            return Err(ApiError::no_encontrado("Orden", order_id));
        };
        let comprador_id = crate::db::uuid_texto(&orden, "comprador_id").map_err(error_db)?;
        autorizar(actor, &comprador_id)?;

        let filas = sqlx::query(
            "SELECT id, order_id, item_id, sku, token, estado, descargas, max_descargas,
                    expira_en, primera_descarga_en, ultima_descarga_en, creado_en
               FROM orders.entregas_digitales
              WHERE order_id = $1
              ORDER BY creado_en, sku",
        )
        .bind(order_uuid)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(entrega_de_fila).collect()
    }

    pub async fn descargar(
        &self,
        token: &str,
        actor: &UsuarioContexto,
    ) -> Result<DescargaDigital, ApiError> {
        let token_uuid = validar_uuid(token, "token")?;
        let fila = sqlx::query(
            "SELECT e.id, e.order_id, e.item_id, e.sku, e.token, e.estado, e.descargas,
                    e.max_descargas, e.expira_en, e.primera_descarga_en,
                    e.ultima_descarga_en, e.creado_en, e.recurso_url, e.comprador_id,
                    o.estado AS order_estado
               FROM orders.entregas_digitales e
               JOIN orders.orden_vista o ON o.id = e.order_id
              WHERE e.token = $1",
        )
        .bind(token_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Entrega digital", token));
        };
        let comprador_id = crate::db::uuid_texto(&fila, "comprador_id").map_err(error_db)?;
        autorizar(actor, &comprador_id)?;

        let estado: String = fila.try_get("estado").map_err(error_db)?;
        if estado != "activo" {
            return Err(ApiError::nueva(
                "ENTREGA_REVOCADA",
                "El acceso digital esta revocado.",
                axum::http::StatusCode::CONFLICT,
            ));
        }
        let order_estado: String = fila.try_get("order_estado").map_err(error_db)?;
        if !ESTADOS_PAGADOS.contains(&order_estado.as_str()) {
            return Err(ApiError::nueva(
                "ORDEN_NO_PAGADA",
                "La orden aun no esta pagada; la entrega digital no esta disponible.",
                axum::http::StatusCode::CONFLICT,
            ));
        }
        let expira_en: Option<chrono::DateTime<chrono::Utc>> =
            fila.try_get("expira_en").map_err(error_db)?;
        if expira_en.is_some_and(|fecha| fecha <= chrono::Utc::now()) {
            return Err(ApiError::nueva(
                "ENTREGA_EXPIRADA",
                "El acceso digital expiro.",
                axum::http::StatusCode::CONFLICT,
            ));
        }
        let descargas: i32 = fila.try_get("descargas").map_err(error_db)?;
        let max_descargas: Option<i32> = fila.try_get("max_descargas").map_err(error_db)?;
        if max_descargas.is_some_and(|maximo| descargas >= maximo) {
            return Err(ApiError::nueva(
                "ENTREGA_SIN_DESCARGAS",
                "El acceso digital alcanzo el maximo de descargas.",
                axum::http::StatusCode::CONFLICT,
            ));
        }

        let actualizada = sqlx::query(
            "UPDATE orders.entregas_digitales
                SET descargas = descargas + 1,
                    primera_descarga_en = COALESCE(primera_descarga_en, NOW()),
                    ultima_descarga_en = NOW()
              WHERE token = $1 AND estado = 'activo'
                AND (max_descargas IS NULL OR descargas < max_descargas)
                AND (expira_en IS NULL OR expira_en > NOW())
              RETURNING id, order_id, item_id, sku, token, estado, descargas,
                        max_descargas, expira_en, primera_descarga_en,
                        ultima_descarga_en, creado_en, recurso_url, comprador_id",
        )
        .bind(token_uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = actualizada else {
            return Err(ApiError::nueva(
                "ENTREGA_SIN_DESCARGAS",
                "El acceso digital ya no esta disponible.",
                axum::http::StatusCode::CONFLICT,
            ));
        };
        descarga_de_fila(&fila)
    }
}

fn autorizar(actor: &UsuarioContexto, comprador_id: &str) -> Result<(), ApiError> {
    if actor.es_admin_global() {
        return Ok(());
    }
    if actor.rol != "comprador" || actor.user_id != comprador_id {
        return Err(ApiError::acceso_denegado(
            "Solo el comprador de la orden puede acceder a la entrega digital.",
        ));
    }
    Ok(())
}

fn validar_uuid(valor: &str, campo: &str) -> Result<uuid::Uuid, ApiError> {
    crate::event_store::parsear_uuid(valor)
        .map_err(|_| ApiError::dominio("ID_INVALIDO", format!("El {campo} debe ser un UUID.")))
}

fn entrega_de_fila(fila: &PgRow) -> Result<EntregaDigital, ApiError> {
    Ok(EntregaDigital {
        id: crate::db::uuid_texto(fila, "id").map_err(error_db)?,
        order_id: crate::db::uuid_texto(fila, "order_id").map_err(error_db)?,
        item_id: crate::db::uuid_texto(fila, "item_id").map_err(error_db)?,
        sku: fila.try_get("sku").map_err(error_db)?,
        token: crate::db::uuid_texto(fila, "token").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        descargas: fila.try_get("descargas").map_err(error_db)?,
        max_descargas: fila.try_get("max_descargas").map_err(error_db)?,
        expira_en: fila
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("expira_en")
            .map_err(error_db)?
            .map(iso_ms),
        primera_descarga_en: fila
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("primera_descarga_en")
            .map_err(error_db)?
            .map(iso_ms),
        ultima_descarga_en: fila
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("ultima_descarga_en")
            .map_err(error_db)?
            .map(iso_ms),
        creado_en: fila.try_get("creado_en").map(iso_ms).map_err(error_db)?,
    })
}

fn descarga_de_fila(fila: &PgRow) -> Result<DescargaDigital, ApiError> {
    Ok(DescargaDigital {
        id: crate::db::uuid_texto(fila, "id").map_err(error_db)?,
        order_id: crate::db::uuid_texto(fila, "order_id").map_err(error_db)?,
        item_id: crate::db::uuid_texto(fila, "item_id").map_err(error_db)?,
        sku: fila.try_get("sku").map_err(error_db)?,
        token: crate::db::uuid_texto(fila, "token").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        descargas: fila.try_get("descargas").map_err(error_db)?,
        max_descargas: fila.try_get("max_descargas").map_err(error_db)?,
        expira_en: fila
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("expira_en")
            .map_err(error_db)?
            .map(iso_ms),
        primera_descarga_en: fila
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("primera_descarga_en")
            .map_err(error_db)?
            .map(iso_ms),
        ultima_descarga_en: fila
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("ultima_descarga_en")
            .map_err(error_db)?
            .map(iso_ms),
        creado_en: fila.try_get("creado_en").map(iso_ms).map_err(error_db)?,
        recurso_url: fila.try_get("recurso_url").map_err(error_db)?,
        comprador_id: crate::db::uuid_texto(fila, "comprador_id").map_err(error_db)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_los_identificadores() {
        assert!(validar_uuid("123e4567-e89b-12d3-a456-426614174000", "token").is_ok());
        assert_eq!(
            validar_uuid("nope", "token").expect_err("invalido").codigo,
            "ID_INVALIDO"
        );
    }

    #[test]
    fn debe_autorizar_solo_al_comprador_o_admin() {
        let comprador = UsuarioContexto {
            user_id: "c-1".into(),
            rol: "comprador".into(),
            ..Default::default()
        };
        assert!(autorizar(&comprador, "c-1").is_ok());
        assert!(autorizar(&comprador, "c-2").is_err());
        let admin = UsuarioContexto {
            user_id: "a-1".into(),
            rol: "admin".into(),
            ..Default::default()
        };
        assert!(autorizar(&admin, "c-2").is_ok());
    }
}
