//! Buckets y cuotas por tenant (ADR-24).

use crate::almacen::{AlmacenError, AlmacenObjetos};
use crate::config::Config;
use crate::db::{error_db, timestamp, uuid_texto, Db};
use crate::errors::ApiError;
use crate::storage::pagination::{crear_pagina, parsear_paginacion, Pagina};
use serde::Serialize;
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EstadoBucket {
    pub tenant_id: String,
    pub bucket: String,
    pub cuota_bytes: i64,
    pub usado_bytes: i64,
    pub disponible_bytes: i64,
    pub objetos: i64,
    pub estado: String,
    pub creado_en: String,
    pub actualizado_en: String,
}

pub struct Buckets {
    db: Arc<Db>,
    almacen: Arc<dyn AlmacenObjetos>,
    config: Arc<Config>,
}

impl Buckets {
    pub fn nueva(db: Arc<Db>, almacen: Arc<dyn AlmacenObjetos>, config: Arc<Config>) -> Self {
        Self {
            db,
            almacen,
            config,
        }
    }

    pub fn nombre_bucket(&self, tenant_id: &str) -> String {
        format!("{}{}", self.config.bucket_prefix, tenant_id).to_lowercase()
    }

    pub async fn asegurar(&self, tenant_id: &str) -> Result<EstadoBucket, ApiError> {
        let bucket = self.nombre_bucket(tenant_id);
        self.almacen
            .asegurar_bucket(&bucket)
            .await
            .map_err(error_almacen)?;
        sqlx::query(
            "INSERT INTO storage.buckets (tenant_id, bucket, cuota_bytes, creado_en, actualizado_en)
             VALUES ($1, $2, $3, NOW(), NOW())
             ON CONFLICT (tenant_id) DO NOTHING",
        )
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .bind(&bucket)
        .bind(self.config.cuota_default_bytes)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        self.estado(tenant_id).await
    }

    pub async fn estado(&self, tenant_id: &str) -> Result<EstadoBucket, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.buckets b WHERE b.tenant_id = $1"
        ))
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Bucket del tenant", tenant_id))?;
        serializar(&fila)
    }

    pub async fn bloquear_estado_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        tenant_id: &str,
    ) -> Result<EstadoBucket, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.buckets b WHERE b.tenant_id = $1 FOR UPDATE"
        ))
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Bucket del tenant", tenant_id))?;
        serializar(&fila)
    }

    pub async fn listar(
        &self,
        pagina: Option<i64>,
        limite: Option<i64>,
    ) -> Result<Pagina<EstadoBucket>, ApiError> {
        let (pagina, limite) = parsear_paginacion(pagina, limite);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM storage.buckets")
            .fetch_one(self.db.pool())
            .await
            .map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.buckets b
             ORDER BY b.creado_en DESC LIMIT $1 OFFSET $2"
        ))
        .bind(limite)
        .bind((pagina - 1) * limite)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        let items = filas
            .iter()
            .map(serializar)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crear_pagina(items, total, pagina, limite))
    }

    pub async fn actualizar_cuota(
        &self,
        tenant_id: &str,
        cuota_bytes: i64,
    ) -> Result<EstadoBucket, ApiError> {
        if cuota_bytes <= 0 {
            return Err(ApiError::dominio(
                "CUOTA_INVALIDA",
                "La cuota debe ser un entero positivo en bytes.",
            ));
        }
        let actual = self.estado(tenant_id).await?;
        if cuota_bytes < actual.usado_bytes {
            return Err(ApiError::dominio(
                "CUOTA_MENOR_AL_USO",
                format!(
                    "La cuota no puede ser menor al uso actual ({} bytes).",
                    actual.usado_bytes
                ),
            )
            .con_detalles(json!({ "usado_bytes": actual.usado_bytes })));
        }
        sqlx::query(
            "UPDATE storage.buckets SET cuota_bytes = $2, actualizado_en = NOW()
             WHERE tenant_id = $1",
        )
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .bind(cuota_bytes)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        self.estado(tenant_id).await
    }

    pub async fn cambiar_estado(
        &self,
        tenant_id: &str,
        estado: &str,
    ) -> Result<EstadoBucket, ApiError> {
        self.estado(tenant_id).await?;
        sqlx::query(
            "UPDATE storage.buckets SET estado = $2, actualizado_en = NOW()
             WHERE tenant_id = $1",
        )
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .bind(estado)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        self.estado(tenant_id).await
    }

    pub fn verificar_cuota(
        &self,
        estado: &EstadoBucket,
        bytes_nuevos: i64,
    ) -> Result<(), ApiError> {
        if estado.estado != "activo" {
            return Err(ApiError::dominio(
                "ALMACENAMIENTO_SUSPENDIDO",
                "El almacenamiento del tenant esta suspendido.",
            ));
        }
        if estado.usado_bytes + bytes_nuevos > estado.cuota_bytes {
            return Err(ApiError::dominio(
                "CUOTA_EXCEDIDA",
                "La operacion excede la cuota de almacenamiento del tenant.",
            )
            .con_detalles(json!({
                "cuota_bytes": estado.cuota_bytes,
                "usado_bytes": estado.usado_bytes,
                "solicitado_bytes": bytes_nuevos,
            })));
        }
        Ok(())
    }

    pub async fn sumar_uso_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        tenant_id: &str,
        bytes: i64,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE storage.buckets SET usado_bytes = usado_bytes + $2, actualizado_en = NOW()
             WHERE tenant_id = $1",
        )
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .bind(bytes)
        .execute(&mut **tx)
        .await
        .map_err(error_db)?;
        Ok(())
    }

    pub async fn restar_uso_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        tenant_id: &str,
        bytes: i64,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "UPDATE storage.buckets
                SET usado_bytes = GREATEST(0, usado_bytes - $2), actualizado_en = NOW()
              WHERE tenant_id = $1",
        )
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .bind(bytes)
        .execute(&mut **tx)
        .await
        .map_err(error_db)?;
        Ok(())
    }

    pub async fn recalcular_uso(&self, tenant_id: &str) -> Result<EstadoBucket, ApiError> {
        sqlx::query(
            "UPDATE storage.buckets b
                SET usado_bytes = COALESCE((
                      SELECT SUM(o.tamano_bytes) FROM storage.objetos o
                       WHERE o.tenant_id = b.tenant_id AND o.estado = 'activo'), 0),
                    actualizado_en = NOW()
              WHERE b.tenant_id = $1",
        )
        .bind(uuid::Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        self.estado(tenant_id).await
    }
}

const COLUMNAS: &str = "b.tenant_id, b.bucket, b.cuota_bytes, b.usado_bytes, b.estado,
  b.creado_en, b.actualizado_en,
  (SELECT COUNT(*)::bigint FROM storage.objetos o
    WHERE o.tenant_id = b.tenant_id AND o.estado = 'activo') AS objetos";

fn serializar(fila: &sqlx::postgres::PgRow) -> Result<EstadoBucket, ApiError> {
    let cuota = fila.try_get::<i64, _>("cuota_bytes").map_err(error_db)?;
    let usado = fila.try_get::<i64, _>("usado_bytes").map_err(error_db)?;
    Ok(EstadoBucket {
        tenant_id: uuid_texto(fila, "tenant_id").map_err(error_db)?,
        bucket: fila.try_get::<String, _>("bucket").map_err(error_db)?,
        cuota_bytes: cuota,
        usado_bytes: usado,
        disponible_bytes: (cuota - usado).max(0),
        objetos: fila.try_get::<i64, _>("objetos").map_err(error_db)?,
        estado: fila.try_get::<String, _>("estado").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

pub fn error_almacen(error: AlmacenError) -> ApiError {
    match error {
        AlmacenError::NoEncontrado => ApiError::no_encontrado("Objeto en el almacenamiento", ""),
        AlmacenError::Operacion(detalle) => ApiError::nueva(
            "ALMACEN_NO_DISPONIBLE",
            detalle,
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
        ),
    }
}
