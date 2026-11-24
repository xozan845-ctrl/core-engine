//! Repositorio de plantillas editables por evento y canal.

use crate::db::Db;
use crate::errors::ErrorNotificacion;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plantilla {
    pub empresa_id: Option<String>,
    pub evento: String,
    pub canal: String,
    pub asunto: String,
    pub cuerpo: String,
    pub activo: bool,
}

pub struct Plantillas {
    db: Arc<Db>,
}

impl Plantillas {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Devuelve la plantilla del tenant si existe; si no, la global.
    pub async fn obtener(
        &self,
        empresa_id: Option<&str>,
        evento: &str,
        canal: &str,
    ) -> Result<Option<Plantilla>, ErrorNotificacion> {
        let fila = sqlx::query(
            "SELECT empresa_id, evento, canal, asunto, cuerpo, activo
               FROM notifications.plantillas
              WHERE evento = $1 AND canal = $2 AND activo
                AND (empresa_id = $3::uuid OR empresa_id IS NULL)
              ORDER BY (empresa_id IS NOT NULL) DESC
              LIMIT 1",
        )
        .bind(evento)
        .bind(canal)
        .bind(empresa_id)
        .fetch_optional(self.db.pool())
        .await?;
        fila.as_ref().map(fila_plantilla).transpose()
    }

    pub async fn listar(&self, empresa_id: Option<&str>) -> Result<Value, ErrorNotificacion> {
        let filas = sqlx::query(
            "SELECT empresa_id, evento, canal, asunto, cuerpo, activo
               FROM notifications.plantillas
              WHERE empresa_id = $1::uuid OR empresa_id IS NULL
              ORDER BY evento, canal, (empresa_id IS NOT NULL) DESC",
        )
        .bind(empresa_id)
        .fetch_all(self.db.pool())
        .await?;
        let datos = filas
            .iter()
            .map(fila_plantilla)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|plantilla| {
                json!({
                    "empresa_id": plantilla.empresa_id,
                    "evento": plantilla.evento,
                    "canal": plantilla.canal,
                    "asunto": plantilla.asunto,
                    "cuerpo": plantilla.cuerpo,
                    "activo": plantilla.activo,
                })
            })
            .collect::<Vec<_>>();
        Ok(Value::Array(datos))
    }

    /// Exportación de tenant: solo las plantillas propias (las globales son plataforma).
    pub async fn exportar(&self, empresa_id: &str) -> Result<Value, ErrorNotificacion> {
        let fila = sqlx::query(
            "SELECT COALESCE(jsonb_agg(to_jsonb(p) ORDER BY p.evento, p.canal), '[]'::jsonb) AS datos
               FROM notifications.plantillas p
              WHERE p.empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_one(self.db.pool())
        .await?;
        fila.try_get("datos").map_err(Into::into)
    }

    /// Purga las plantillas propias del tenant (las globales se conservan).
    pub async fn purgar(&self, empresa_id: &str) -> Result<u64, ErrorNotificacion> {
        let resultado =
            sqlx::query("DELETE FROM notifications.plantillas WHERE empresa_id = $1::uuid")
                .bind(empresa_id)
                .execute(self.db.pool())
                .await?;
        Ok(resultado.rows_affected())
    }

    pub async fn guardar(
        &self,
        empresa_id: Option<&str>,
        evento: &str,
        canal: &str,
        asunto: &str,
        cuerpo: &str,
        activo: bool,
    ) -> Result<Plantilla, ErrorNotificacion> {
        if !matches!(canal, "email" | "whatsapp" | "sms") {
            return Err(ErrorNotificacion::Evento(
                "El canal debe ser email, whatsapp o sms.".to_string(),
            ));
        }
        let fila = sqlx::query(
            "INSERT INTO notifications.plantillas
               (empresa_id, evento, canal, asunto, cuerpo, activo)
             VALUES ($1::uuid, $2, $3, $4, $5, $6)
             ON CONFLICT (COALESCE(empresa_id, '00000000-0000-0000-0000-000000000000'::uuid), evento, canal)
             DO UPDATE SET asunto = EXCLUDED.asunto, cuerpo = EXCLUDED.cuerpo,
                           activo = EXCLUDED.activo, actualizado_en = NOW()
             RETURNING empresa_id, evento, canal, asunto, cuerpo, activo",
        )
        .bind(empresa_id)
        .bind(evento)
        .bind(canal)
        .bind(asunto)
        .bind(cuerpo)
        .bind(activo)
        .fetch_one(self.db.pool())
        .await?;
        fila_plantilla(&fila)
    }
}

fn fila_plantilla(fila: &sqlx::postgres::PgRow) -> Result<Plantilla, ErrorNotificacion> {
    Ok(Plantilla {
        empresa_id: fila
            .try_get::<Option<uuid::Uuid>, _>("empresa_id")?
            .map(|valor| valor.to_string()),
        evento: fila.try_get("evento")?,
        canal: fila.try_get("canal")?,
        asunto: fila.try_get("asunto")?,
        cuerpo: fila.try_get("cuerpo")?,
        activo: fila.try_get("activo")?,
    })
}
