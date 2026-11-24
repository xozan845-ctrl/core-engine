//! Onboarding idempotente de empresas (ADR-0047).
//!
//! El job guarda la clave, el hash del cuerpo, el slug elegido y el estado.
//! La creación de la empresa sigue siendo la transacción de `Empresas::crear`;
//! este módulo solo aporta idempotencia, recuperación y estado observable.

use crate::db::Db;
use crate::errors::ApiError;
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::sync::Arc;

const ESTADOS: [&str; 3] = ["en_proceso", "completado", "fallido"];

#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: String,
    pub request_hash: String,
    pub slug: String,
    pub estado: String,
    pub paso: String,
    pub empresa_id: Option<String>,
    pub negocio_id: Option<String>,
    pub resultado: Value,
    pub error: String,
}

#[derive(Debug)]
pub enum Inicio {
    Nuevo(Job),
    Replay(Job),
    Reclamado(Job),
}

pub struct Onboarding {
    db: Arc<Db>,
}

impl Onboarding {
    pub fn nuevo(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Registra o recupera un job. Serializa por la clave única del actor.
    pub async fn iniciar(
        &self,
        actor_id: &str,
        clave: &str,
        request_hash: &str,
        slug: &str,
    ) -> Result<Inicio, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let insertado: Option<Value> = sqlx::query_scalar(
            "INSERT INTO identity.provisioning_jobs
               (actor_id, idempotency_key, request_hash, slug)
             VALUES ($1::uuid, $2, $3, $4)
             ON CONFLICT (actor_id, idempotency_key) DO NOTHING
             RETURNING to_jsonb(provisioning_jobs) AS datos",
        )
        .bind(actor_id)
        .bind(clave)
        .bind(request_hash)
        .bind(slug)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if let Some(datos) = insertado {
            tx.commit().await.map_err(interno)?;
            return Ok(Inicio::Nuevo(job_de(&datos)?));
        }

        let datos: Value = sqlx::query_scalar(
            "SELECT to_jsonb(j) AS datos
               FROM identity.provisioning_jobs j
              WHERE j.actor_id = $1::uuid AND j.idempotency_key = $2
              FOR UPDATE",
        )
        .bind(actor_id)
        .bind(clave)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let job = job_de(&datos)?;
        if job.request_hash != request_hash {
            return Err(ApiError::nueva(
                "IDEMPOTENCIA_CLAVE_REUTILIZADA",
                "La Idempotency-Key ya se usó con otro cuerpo.",
                StatusCode::UNPROCESSABLE_ENTITY,
            ));
        }
        match job.estado.as_str() {
            "completado" => {
                tx.commit().await.map_err(interno)?;
                Ok(Inicio::Replay(job))
            }
            "en_proceso" if job.empresa_id.is_some() => {
                let actualizado: Value = sqlx::query_scalar(
                    "UPDATE identity.provisioning_jobs
                        SET estado = 'completado', paso = 'completado', actualizado_en = NOW()
                      WHERE id = $1::uuid
                      RETURNING to_jsonb(provisioning_jobs) AS datos",
                )
                .bind(&job.id)
                .fetch_one(&mut *tx)
                .await
                .map_err(interno)?;
                tx.commit().await.map_err(interno)?;
                Ok(Inicio::Replay(job_de(&actualizado)?))
            }
            "en_proceso" => {
                let reclamado: Option<Value> = sqlx::query_scalar(
                    "UPDATE identity.provisioning_jobs
                        SET actualizado_en = NOW(), error = ''
                      WHERE id = $1::uuid AND estado = 'en_proceso'
                        AND actualizado_en < NOW() - INTERVAL '2 minutes'
                      RETURNING to_jsonb(provisioning_jobs) AS datos",
                )
                .bind(&job.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?;
                match reclamado {
                    Some(datos) => {
                        tx.commit().await.map_err(interno)?;
                        Ok(Inicio::Reclamado(job_de(&datos)?))
                    }
                    None => Err(ApiError::nueva(
                        "IDEMPOTENCIA_EN_PROCESO",
                        "Ya hay una solicitud con esta Idempotency-Key en proceso.",
                        StatusCode::CONFLICT,
                    )),
                }
            }
            "fallido" => {
                let reclamado: Value = sqlx::query_scalar(
                    "UPDATE identity.provisioning_jobs
                        SET estado = 'en_proceso', error = '', actualizado_en = NOW()
                      WHERE id = $1::uuid
                      RETURNING to_jsonb(provisioning_jobs) AS datos",
                )
                .bind(&job.id)
                .fetch_one(&mut *tx)
                .await
                .map_err(interno)?;
                tx.commit().await.map_err(interno)?;
                Ok(Inicio::Reclamado(job_de(&reclamado)?))
            }
            otro => Err(ApiError::interno().con_detalles(json!({
                "motivo": "estado de onboarding desconocido",
                "estado": otro,
            }))),
        }
    }

    pub async fn completar(
        &self,
        id: &str,
        empresa_id: &str,
        negocio_id: &str,
        resultado: Value,
    ) -> Result<Job, ApiError> {
        let datos: Value = sqlx::query_scalar(
            "UPDATE identity.provisioning_jobs
                SET estado = 'completado', paso = 'completado',
                    empresa_id = $2::uuid, negocio_id = $3::uuid,
                    resultado = $4, error = '', actualizado_en = NOW()
              WHERE id = $1::uuid
              RETURNING to_jsonb(provisioning_jobs) AS datos",
        )
        .bind(id)
        .bind(empresa_id)
        .bind(negocio_id)
        .bind(resultado)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Onboarding", id))?;
        job_de(&datos)
    }

    pub async fn fallar(&self, id: &str, error: &str) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE identity.provisioning_jobs
                SET estado = 'fallido', error = $2, actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(id)
        .bind(error.chars().take(500).collect::<String>())
        .execute(self.db.pool())
        .await
        .map_err(interno)?;
        Ok(())
    }

    pub async fn obtener(&self, actor_id: &str, id: &str) -> Result<Job, ApiError> {
        let datos: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(j) AS datos
               FROM identity.provisioning_jobs j
              WHERE j.id = $1::uuid AND j.actor_id = $2::uuid",
        )
        .bind(id)
        .bind(actor_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?;
        datos
            .as_ref()
            .map(job_de)
            .transpose()?
            .ok_or_else(|| ApiError::no_encontrado("Onboarding", id))
    }

    pub async fn empresa_id_por_slug(
        &self,
        actor_id: &str,
        slug: &str,
    ) -> Result<Option<String>, ApiError> {
        sqlx::query_scalar(
            "SELECT id::text FROM identity.empresas
              WHERE slug = $1 AND creado_por = $2::uuid",
        )
        .bind(slug)
        .bind(actor_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)
    }

    pub async fn negocio_principal(&self, empresa_id: &str) -> Result<String, ApiError> {
        sqlx::query_scalar(
            "SELECT id::text FROM identity.negocios
              WHERE empresa_id = $1::uuid AND es_predeterminado
              ORDER BY creado_en LIMIT 1",
        )
        .bind(empresa_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::dominio(
                "NEGOCIO_PRINCIPAL_AUSENTE",
                "La empresa no tiene negocio principal.",
            )
        })
    }

    pub async fn capacidades(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let datos: Value = sqlx::query_scalar(
            "SELECT COALESCE(
                jsonb_agg(capacidad_codigo ORDER BY capacidad_codigo),
                '[]'::jsonb
             ) AS datos
               FROM identity.empresa_capacidades
              WHERE empresa_id = $1::uuid AND activo",
        )
        .bind(empresa_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(interno)?;
        Ok(datos)
    }
}

fn job_de(datos: &Value) -> Result<Job, ApiError> {
    let texto = |clave: &str| {
        datos
            .get(clave)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let estado = texto("estado");
    if !ESTADOS.contains(&estado.as_str()) {
        return Err(ApiError::interno());
    }
    Ok(Job {
        id: texto("id"),
        request_hash: texto("request_hash"),
        slug: texto("slug"),
        estado,
        paso: texto("paso"),
        empresa_id: datos
            .get("empresa_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        negocio_id: datos
            .get("negocio_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        resultado: datos.get("resultado").cloned().unwrap_or_else(|| json!({})),
        error: texto("error"),
    })
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}
