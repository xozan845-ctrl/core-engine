//! Fidelización (capacidad `fidelizacion`): programas, tarjetas, movimientos,
//! recompensas y canjes.
//!
//! El programa es único por empresa. Las tarjetas se emiten por parte
//! (`identity.partes`) y los movimientos registran acumulaciones, canjes,
//! ajustes y expiraciones. La acumulación por orden es idempotente gracias al
//! índice único `(tarjeta_id, order_id)` de los movimientos de acumulación.

use crate::db::{jsonb, timestamp, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Programa {
    pub id: String,
    pub empresa_id: String,
    pub nombre: String,
    pub activo: bool,
    /// Puntos otorgados por cada unidad de moneda (100 centavos).
    pub puntos_por_unidad: i32,
    pub expira_meses: Option<i32>,
    pub config: Value,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tarjeta {
    pub id: String,
    pub empresa_id: String,
    pub programa_id: String,
    pub parte_id: String,
    pub parte_nombre: String,
    pub codigo: String,
    pub puntos: i32,
    pub estado: String,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Movimiento {
    pub id: String,
    pub empresa_id: String,
    pub tarjeta_id: String,
    pub tipo: String,
    pub puntos: i32,
    pub saldo_tras: i32,
    pub order_id: Option<String>,
    pub event_id: Option<String>,
    pub descripcion: String,
    pub creado_por: Option<String>,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recompensa {
    pub id: String,
    pub empresa_id: String,
    pub codigo: String,
    pub nombre: String,
    pub descripcion: String,
    pub puntos: i32,
    pub stock: Option<i32>,
    pub activo: bool,
    pub config: Value,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Canje {
    pub id: String,
    pub empresa_id: String,
    pub tarjeta_id: String,
    pub recompensa_id: String,
    pub movimiento_id: String,
    pub order_id: Option<String>,
    pub estado: String,
    pub puntos: i32,
    pub creado_por: Option<String>,
    pub creado_en: String,
}

pub struct Fidelizacion {
    db: Arc<Db>,
}

const COLUMNAS_PROGRAMA: &str =
    "id, empresa_id, nombre, activo, puntos_por_unidad, expira_meses, config,
     creado_en, actualizado_en";

const COLUMNAS_TARJETA: &str =
    "t.id, t.empresa_id, t.programa_id, t.parte_id, p.nombre AS parte_nombre,
     t.codigo, t.puntos, t.estado, t.creado_en, t.actualizado_en";

const COLUMNAS_MOVIMIENTO: &str =
    "id, empresa_id, tarjeta_id, tipo, puntos, saldo_tras, order_id, event_id,
     descripcion, creado_por, creado_en";

const COLUMNAS_RECOMPENSA: &str =
    "id, empresa_id, codigo, nombre, descripcion, puntos, stock, activo, config,
     creado_en, actualizado_en";

const COLUMNAS_CANJE: &str =
    "id, empresa_id, tarjeta_id, recompensa_id, movimiento_id, order_id, estado,
     puntos, creado_por, creado_en";

impl Fidelizacion {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn programa(&self, empresa_id: &str) -> Result<Option<Programa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PROGRAMA} FROM orders.fidelizacion_programas
              WHERE empresa_id = $1::uuid"
        ))
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.as_ref().map(fila_programa).transpose()
    }

    pub async fn guardar_programa(
        &self,
        empresa_id: &str,
        dto: &Value,
    ) -> Result<Programa, ApiError> {
        let nombre = texto(dto, "nombre");
        if nombre.chars().count() < 2 || nombre.chars().count() > 120 {
            return Err(ApiError::validacion(json!([
                {"campo": "nombre", "restricciones": ["isLength"]}
            ])));
        }
        let activo = dto.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let puntos_por_unidad = entero(dto, "puntos_por_unidad").unwrap_or(1);
        if !(0..=100_000).contains(&puntos_por_unidad) {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos_por_unidad", "restricciones": ["min", "max"]}
            ])));
        }
        let expira_meses = entero(dto, "expira_meses");
        if expira_meses.is_some_and(|meses| meses <= 0) {
            return Err(ApiError::validacion(json!([
                {"campo": "expira_meses", "restricciones": ["min"]}
            ])));
        }
        let config = dto
            .get("config")
            .filter(|valor| valor.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_programas AS f
               (empresa_id, nombre, activo, puntos_por_unidad, expira_meses, config,
                actualizado_en)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, NOW())
             ON CONFLICT (empresa_id) DO UPDATE
               SET nombre = EXCLUDED.nombre,
                   activo = EXCLUDED.activo,
                   puntos_por_unidad = EXCLUDED.puntos_por_unidad,
                   expira_meses = EXCLUDED.expira_meses,
                   config = EXCLUDED.config,
                   actualizado_en = NOW()
             RETURNING {COLUMNAS_PROGRAMA}"
        ))
        .bind(empresa_id)
        .bind(&nombre)
        .bind(activo)
        .bind(puntos_por_unidad)
        .bind(expira_meses)
        .bind(&config)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_programa(&fila)
    }

    pub async fn listar_tarjetas(
        &self,
        empresa_id: &str,
        parte_id: Option<&str>,
    ) -> Result<Vec<Tarjeta>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.empresa_id = $1::uuid
                AND ($2::text IS NULL OR t.parte_id = $2::uuid)
              ORDER BY t.actualizado_en DESC"
        ))
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_tarjeta).collect()
    }

    pub async fn obtener_tarjeta(
        &self,
        empresa_id: &str,
        tarjeta_id: &str,
    ) -> Result<Tarjeta, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.empresa_id = $1::uuid AND t.id = $2::uuid"
        ))
        .bind(empresa_id)
        .bind(tarjeta_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.as_ref()
            .map(fila_tarjeta)
            .transpose()?
            .ok_or_else(|| ApiError::no_encontrado("Tarjeta", tarjeta_id))
    }

    /// Tarjetas de un comprador: solo las partes que aparecen en sus propias
    /// órdenes (evita exponer tarjetas de otros clientes del mismo tenant).
    pub async fn listar_tarjetas_de_comprador(
        &self,
        comprador_id: &str,
    ) -> Result<Vec<Tarjeta>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.parte_id IN (
                    SELECT DISTINCT parte_id FROM orders.orden_vista
                     WHERE comprador_id = $1::uuid AND parte_id IS NOT NULL)
              ORDER BY t.actualizado_en DESC"
        ))
        .bind(comprador_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_tarjeta).collect()
    }

    pub async fn movimientos_de_comprador(
        &self,
        comprador_id: &str,
        tarjeta_id: &str,
        limite: i64,
    ) -> Result<Vec<Movimiento>, ApiError> {
        let limite = limite.clamp(1, 200);
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let propia = sqlx::query(
            "SELECT 1 FROM orders.fidelizacion_tarjetas t
              WHERE t.id = $1::uuid
                AND t.parte_id IN (
                    SELECT DISTINCT parte_id FROM orders.orden_vista
                     WHERE comprador_id = $2::uuid AND parte_id IS NOT NULL)",
        )
        .bind(tarjeta_id)
        .bind(comprador_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if propia.is_none() {
            return Err(ApiError::no_encontrado("Tarjeta", tarjeta_id));
        }
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_MOVIMIENTO}
               FROM orders.fidelizacion_movimientos
              WHERE tarjeta_id = $1::uuid
              ORDER BY creado_en DESC
              LIMIT $2"
        ))
        .bind(tarjeta_id)
        .bind(limite)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_movimiento).collect()
    }

    pub async fn crear_tarjeta(&self, empresa_id: &str, dto: &Value) -> Result<Tarjeta, ApiError> {
        let parte_id = texto(dto, "parte_id");
        if parte_id.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "parte_id", "restricciones": ["isUuid"]}
            ])));
        }
        let codigo = match texto(dto, "codigo") {
            valor if valor.is_empty() => None,
            valor => Some(valor.to_uppercase()),
        };
        if let Some(codigo) = codigo.as_deref() {
            if !codigo_valido(codigo) {
                return Err(ApiError::validacion(json!([
                    {"campo": "codigo", "restricciones": ["matches"]}
                ])));
            }
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let programa = sqlx::query(
            "SELECT id FROM orders.fidelizacion_programas
              WHERE empresa_id = $1::uuid AND activo",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "PROGRAMA_NO_CONFIGURADO",
                "Configura y activa el programa de fidelización antes de emitir tarjetas.",
            )
        })?;
        let programa_id = uuid_texto(&programa, "id").map_err(error_db)?;
        let parte = sqlx::query(
            "SELECT 1 FROM identity.partes
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(&parte_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if parte.is_none() {
            return Err(ApiError::no_encontrado("Parte", &parte_id));
        }
        let codigo = codigo.unwrap_or_else(|| {
            format!(
                "FID-{}",
                uuid::Uuid::new_v4().simple().to_string()[..8].to_uppercase()
            )
        });
        let insertada = sqlx::query(
            "INSERT INTO orders.fidelizacion_tarjetas
               (empresa_id, programa_id, parte_id, codigo)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4)
             RETURNING id",
        )
        .bind(empresa_id)
        .bind(&programa_id)
        .bind(&parte_id)
        .bind(&codigo)
        .fetch_optional(&mut *tx)
        .await;
        let fila = match insertada {
            Ok(fila) => fila,
            Err(error) if es_unico(&error) => {
                return Err(ApiError::conflicto(
                    "El cliente ya tiene una tarjeta o el código está en uso.",
                ));
            }
            Err(error) => return Err(error_db(error)),
        }
        .ok_or_else(ApiError::interno)?;
        let tarjeta_id = uuid_texto(&fila, "id").map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_TARJETA}
               FROM orders.fidelizacion_tarjetas t
               JOIN identity.partes p ON p.id = t.parte_id
              WHERE t.id = $1::uuid"
        ))
        .bind(&tarjeta_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_tarjeta(&fila)
    }

    pub async fn listar_movimientos(
        &self,
        empresa_id: &str,
        tarjeta_id: &str,
        limite: i64,
    ) -> Result<Vec<Movimiento>, ApiError> {
        let limite = limite.clamp(1, 200);
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_MOVIMIENTO}
               FROM orders.fidelizacion_movimientos
              WHERE empresa_id = $1::uuid AND tarjeta_id = $2::uuid
              ORDER BY creado_en DESC
              LIMIT $3"
        ))
        .bind(empresa_id)
        .bind(tarjeta_id)
        .bind(limite)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_movimiento).collect()
    }

    /// Ajuste manual de puntos (positivo o negativo). El saldo nunca baja de 0.
    pub async fn ajustar(
        &self,
        empresa_id: &str,
        tarjeta_id: &str,
        dto: &Value,
        actor_id: &str,
    ) -> Result<Movimiento, ApiError> {
        let puntos = entero(dto, "puntos").unwrap_or_default();
        if puntos == 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos", "restricciones": ["isInt", "notEquals"]}
            ])));
        }
        let descripcion = texto(dto, "descripcion");
        if descripcion.chars().count() > 500 {
            return Err(ApiError::validacion(json!([
                {"campo": "descripcion", "restricciones": ["maxLength"]}
            ])));
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let tarjeta = sqlx::query(
            "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
              WHERE empresa_id = $1::uuid AND id = $2::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .bind(tarjeta_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Tarjeta", tarjeta_id))?;
        let estado: String = tarjeta.try_get("estado").map_err(error_db)?;
        if estado != "activa" {
            return Err(ApiError::dominio(
                "TARJETA_NO_ACTIVA",
                "La tarjeta no está activa.",
            ));
        }
        let actuales: i32 = tarjeta.try_get("puntos").map_err(error_db)?;
        let saldo = actuales + puntos;
        if saldo < 0 {
            return Err(ApiError::dominio(
                "PUNTOS_INSUFICIENTES",
                "El ajuste dejaría la tarjeta con saldo negativo.",
            ));
        }
        sqlx::query(
            "UPDATE orders.fidelizacion_tarjetas
                SET puntos = $3, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid",
        )
        .bind(empresa_id)
        .bind(tarjeta_id)
        .bind(saldo)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_movimientos
               (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, descripcion, creado_por)
             VALUES ($1::uuid, $2::uuid, 'ajuste', $3, $4, $5, $6::uuid)
             RETURNING {COLUMNAS_MOVIMIENTO}"
        ))
        .bind(empresa_id)
        .bind(tarjeta_id)
        .bind(puntos)
        .bind(saldo)
        .bind(&descripcion)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_movimiento(&fila)
    }

    pub async fn listar_recompensas(&self, empresa_id: &str) -> Result<Vec<Recompensa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_RECOMPENSA}
               FROM orders.fidelizacion_recompensas
              WHERE empresa_id = $1::uuid
              ORDER BY activo DESC, puntos, codigo"
        ))
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recompensa).collect()
    }

    pub async fn crear_recompensa(
        &self,
        empresa_id: &str,
        dto: &Value,
    ) -> Result<Recompensa, ApiError> {
        let codigo = texto(dto, "codigo").to_lowercase();
        if !codigo_recompensa_valido(&codigo) {
            return Err(ApiError::validacion(json!([
                {"campo": "codigo", "restricciones": ["matches"]}
            ])));
        }
        let nombre = texto(dto, "nombre");
        if nombre.chars().count() < 2 || nombre.chars().count() > 120 {
            return Err(ApiError::validacion(json!([
                {"campo": "nombre", "restricciones": ["isLength"]}
            ])));
        }
        let descripcion = texto(dto, "descripcion");
        if descripcion.chars().count() > 500 {
            return Err(ApiError::validacion(json!([
                {"campo": "descripcion", "restricciones": ["maxLength"]}
            ])));
        }
        let puntos = entero(dto, "puntos").unwrap_or_default();
        if puntos <= 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos", "restricciones": ["min"]}
            ])));
        }
        let stock = entero(dto, "stock");
        if stock.is_some_and(|stock| stock < 0) {
            return Err(ApiError::validacion(json!([
                {"campo": "stock", "restricciones": ["min"]}
            ])));
        }
        let activo = dto.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = dto
            .get("config")
            .filter(|valor| valor.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let insertada = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_recompensas
               (empresa_id, codigo, nombre, descripcion, puntos, stock, activo, config)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8)
             RETURNING {COLUMNAS_RECOMPENSA}"
        ))
        .bind(empresa_id)
        .bind(&codigo)
        .bind(&nombre)
        .bind(&descripcion)
        .bind(puntos)
        .bind(stock)
        .bind(activo)
        .bind(&config)
        .fetch_optional(&mut *tx)
        .await;
        let fila = match insertada {
            Ok(fila) => fila,
            Err(error) if es_unico(&error) => {
                return Err(ApiError::conflicto(
                    "Ya existe una recompensa con ese código.",
                ));
            }
            Err(error) => return Err(error_db(error)),
        }
        .ok_or_else(ApiError::interno)?;
        tx.commit().await.map_err(error_db)?;
        fila_recompensa(&fila)
    }

    pub async fn actualizar_recompensa(
        &self,
        empresa_id: &str,
        recompensa_id: &str,
        dto: &Value,
    ) -> Result<Recompensa, ApiError> {
        let actual = {
            let mut tx = self.db.transaccion().await.map_err(error_db)?;
            let fila = sqlx::query(&format!(
                "SELECT {COLUMNAS_RECOMPENSA}
                   FROM orders.fidelizacion_recompensas
                  WHERE empresa_id = $1::uuid AND id = $2::uuid"
            ))
            .bind(empresa_id)
            .bind(recompensa_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Recompensa", recompensa_id))?;
            tx.commit().await.map_err(error_db)?;
            fila_recompensa(&fila)?
        };
        let nombre = dto
            .get("nombre")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string)
            .unwrap_or(actual.nombre);
        if nombre.chars().count() < 2 || nombre.chars().count() > 120 {
            return Err(ApiError::validacion(json!([
                {"campo": "nombre", "restricciones": ["isLength"]}
            ])));
        }
        let descripcion = dto
            .get("descripcion")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(actual.descripcion);
        if descripcion.chars().count() > 500 {
            return Err(ApiError::validacion(json!([
                {"campo": "descripcion", "restricciones": ["maxLength"]}
            ])));
        }
        let puntos = entero(dto, "puntos").unwrap_or(actual.puntos);
        if puntos <= 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "puntos", "restricciones": ["min"]}
            ])));
        }
        let stock = if dto.get("stock").is_some() {
            entero(dto, "stock")
        } else {
            actual.stock
        };
        if stock.is_some_and(|stock| stock < 0) {
            return Err(ApiError::validacion(json!([
                {"campo": "stock", "restricciones": ["min"]}
            ])));
        }
        let activo = dto
            .get("activo")
            .and_then(Value::as_bool)
            .unwrap_or(actual.activo);
        let config = dto
            .get("config")
            .filter(|valor| valor.is_object())
            .cloned()
            .unwrap_or(actual.config);
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(&format!(
            "UPDATE orders.fidelizacion_recompensas
                SET nombre = $3, descripcion = $4, puntos = $5, stock = $6,
                    activo = $7, config = $8, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid
              RETURNING {COLUMNAS_RECOMPENSA}"
        ))
        .bind(empresa_id)
        .bind(recompensa_id)
        .bind(&nombre)
        .bind(&descripcion)
        .bind(puntos)
        .bind(stock)
        .bind(activo)
        .bind(&config)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_recompensa(&fila)
    }

    /// Canjea una recompensa por una tarjeta en una sola transacción.
    pub async fn canjear(
        &self,
        empresa_id: &str,
        dto: &Value,
        actor_id: &str,
    ) -> Result<Canje, ApiError> {
        let tarjeta_ref = texto(dto, "tarjeta_id");
        let tarjeta_codigo = texto(dto, "codigo_tarjeta").to_uppercase();
        if tarjeta_ref.is_empty() && tarjeta_codigo.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "tarjeta_id", "restricciones": ["isUuid"]}
            ])));
        }
        let recompensa_ref = texto(dto, "recompensa_id");
        let recompensa_codigo = texto(dto, "recompensa_codigo").to_lowercase();
        if recompensa_ref.is_empty() && recompensa_codigo.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "recompensa_id", "restricciones": ["isUuid"]}
            ])));
        }
        let order_id = dto
            .get("order_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string);

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let tarjeta = if tarjeta_ref.is_empty() {
            sqlx::query(
                "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
                  WHERE empresa_id = $1::uuid AND codigo = $2 FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&tarjeta_codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Tarjeta", &tarjeta_codigo))?
        } else {
            sqlx::query(
                "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
                  WHERE empresa_id = $1::uuid AND id = $2::uuid FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&tarjeta_ref)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Tarjeta", &tarjeta_ref))?
        };
        let tarjeta_id = uuid_texto(&tarjeta, "id").map_err(error_db)?;
        let estado: String = tarjeta.try_get("estado").map_err(error_db)?;
        if estado != "activa" {
            return Err(ApiError::dominio(
                "TARJETA_NO_ACTIVA",
                "La tarjeta no está activa.",
            ));
        }
        let recompensa = if recompensa_ref.is_empty() {
            sqlx::query(
                "SELECT id, puntos, stock, activo FROM orders.fidelizacion_recompensas
                  WHERE empresa_id = $1::uuid AND codigo = $2 FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&recompensa_codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Recompensa", &recompensa_codigo))?
        } else {
            sqlx::query(
                "SELECT id, puntos, stock, activo FROM orders.fidelizacion_recompensas
                  WHERE empresa_id = $1::uuid AND id = $2::uuid FOR UPDATE",
            )
            .bind(empresa_id)
            .bind(&recompensa_ref)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
            .ok_or_else(|| ApiError::no_encontrado("Recompensa", &recompensa_ref))?
        };
        let recompensa_id = uuid_texto(&recompensa, "id").map_err(error_db)?;
        let recompensa_activa: bool = recompensa.try_get("activo").map_err(error_db)?;
        if !recompensa_activa {
            return Err(ApiError::dominio(
                "RECOMPENSA_NO_ACTIVA",
                "La recompensa no está disponible.",
            ));
        }
        let stock: Option<i32> = recompensa.try_get("stock").map_err(error_db)?;
        if stock.is_some_and(|stock| stock <= 0) {
            return Err(ApiError::dominio(
                "RECOMPENSA_SIN_STOCK",
                "La recompensa no tiene stock disponible.",
            ));
        }
        let costo: i32 = recompensa.try_get("puntos").map_err(error_db)?;
        let actuales: i32 = tarjeta.try_get("puntos").map_err(error_db)?;
        if actuales < costo {
            return Err(ApiError::dominio(
                "PUNTOS_INSUFICIENTES",
                "La tarjeta no tiene puntos suficientes para el canje.",
            ));
        }
        let saldo = actuales - costo;
        sqlx::query(
            "UPDATE orders.fidelizacion_tarjetas
                SET puntos = $3, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid",
        )
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(saldo)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        if stock.is_some() {
            sqlx::query(
                "UPDATE orders.fidelizacion_recompensas
                    SET stock = stock - 1, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND id = $2::uuid",
            )
            .bind(empresa_id)
            .bind(&recompensa_id)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }
        let movimiento = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_movimientos
               (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, order_id,
                descripcion, creado_por)
             VALUES ($1::uuid, $2::uuid, 'canje', $3, $4, $5::uuid, $6, $7::uuid)
             RETURNING {COLUMNAS_MOVIMIENTO}"
        ))
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(-costo)
        .bind(saldo)
        .bind(&order_id)
        .bind(format!("Canje de recompensa {recompensa_id}"))
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        let movimiento_id = uuid_texto(&movimiento, "id").map_err(error_db)?;
        let canje = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_canjes
               (empresa_id, tarjeta_id, recompensa_id, movimiento_id, order_id,
                puntos, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6, $7::uuid)
             RETURNING {COLUMNAS_CANJE}"
        ))
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(&recompensa_id)
        .bind(&movimiento_id)
        .bind(&order_id)
        .bind(costo)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_canje(&canje)
    }

    /// Acumula puntos por una orden pagada. Idempotente por `(tarjeta, orden)`.
    /// Devuelve `None` cuando no hay programa activo, no hay tarjeta, el total
    /// no genera puntos o la orden ya fue acumulada.
    pub async fn acumular_por_orden(
        &self,
        empresa_id: &str,
        parte_id: &str,
        order_id: &str,
        total_cents: i64,
    ) -> Result<Option<Movimiento>, ApiError> {
        if total_cents <= 0 || parte_id.is_empty() || order_id.is_empty() {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let programa = sqlx::query(
            "SELECT puntos_por_unidad FROM orders.fidelizacion_programas
              WHERE empresa_id = $1::uuid AND activo",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(programa) = programa else {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        };
        let por_unidad: i32 = programa.try_get("puntos_por_unidad").map_err(error_db)?;
        let puntos = (total_cents * i64::from(por_unidad) / 100) as i32;
        if puntos <= 0 {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        }
        let tarjeta = sqlx::query(
            "SELECT id, puntos, estado FROM orders.fidelizacion_tarjetas
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(tarjeta) = tarjeta else {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        };
        let estado: String = tarjeta.try_get("estado").map_err(error_db)?;
        if estado != "activa" {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        }
        let tarjeta_id = uuid_texto(&tarjeta, "id").map_err(error_db)?;
        let actuales: i32 = tarjeta.try_get("puntos").map_err(error_db)?;
        let saldo = actuales + puntos;
        let insertado = sqlx::query(&format!(
            "INSERT INTO orders.fidelizacion_movimientos
               (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, order_id,
                descripcion)
             VALUES ($1::uuid, $2::uuid, 'acumulacion', $3, $4, $5::uuid, $6)
             ON CONFLICT (tarjeta_id, order_id)
               WHERE tipo = 'acumulacion' AND order_id IS NOT NULL
               DO NOTHING
             RETURNING {COLUMNAS_MOVIMIENTO}"
        ))
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(puntos)
        .bind(saldo)
        .bind(order_id)
        .bind("Puntos por orden pagada")
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(fila) = insertado else {
            tx.commit().await.map_err(error_db)?;
            return Ok(None);
        };
        sqlx::query(
            "UPDATE orders.fidelizacion_tarjetas
                SET puntos = $3, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid",
        )
        .bind(empresa_id)
        .bind(&tarjeta_id)
        .bind(saldo)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_movimiento(&fila).map(Some)
    }

    /// Recompensas activas de las empresas donde el comprador tiene tarjeta.
    pub async fn listar_recompensas_de_comprador(
        &self,
        comprador_id: &str,
    ) -> Result<Vec<Recompensa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_RECOMPENSA}
               FROM orders.fidelizacion_recompensas r
              WHERE r.activo
                AND r.empresa_id IN (
                    SELECT t.empresa_id FROM orders.fidelizacion_tarjetas t
                     WHERE t.parte_id IN (
                        SELECT DISTINCT parte_id FROM orders.orden_vista
                         WHERE comprador_id = $1::uuid AND parte_id IS NOT NULL))
              ORDER BY r.puntos, r.codigo"
        ))
        .bind(comprador_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recompensa).collect()
    }

    /// Canje iniciado por el comprador: valida que la tarjeta pertenezca a una
    /// de sus órdenes y delega en `canjear` con la empresa de la tarjeta.
    pub async fn canjear_de_comprador(
        &self,
        comprador_id: &str,
        dto: &Value,
    ) -> Result<Canje, ApiError> {
        let tarjeta_ref = texto(dto, "tarjeta_id");
        let codigo = texto(dto, "codigo_tarjeta").to_uppercase();
        if tarjeta_ref.is_empty() && codigo.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "tarjeta_id", "restricciones": ["isUuid"]}
            ])));
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let empresa: Option<uuid::Uuid> = if tarjeta_ref.is_empty() {
            sqlx::query_scalar(
                "SELECT t.empresa_id FROM orders.fidelizacion_tarjetas t
                  WHERE t.codigo = $1
                    AND t.parte_id IN (
                        SELECT DISTINCT parte_id FROM orders.orden_vista
                         WHERE comprador_id = $2::uuid AND parte_id IS NOT NULL)",
            )
            .bind(&codigo)
            .bind(comprador_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
        } else {
            sqlx::query_scalar(
                "SELECT t.empresa_id FROM orders.fidelizacion_tarjetas t
                  WHERE t.id = $1::uuid
                    AND t.parte_id IN (
                        SELECT DISTINCT parte_id FROM orders.orden_vista
                         WHERE comprador_id = $2::uuid AND parte_id IS NOT NULL)",
            )
            .bind(&tarjeta_ref)
            .bind(comprador_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?
        };
        let Some(empresa) = empresa else {
            let referencia = if tarjeta_ref.is_empty() {
                codigo.as_str()
            } else {
                tarjeta_ref.as_str()
            };
            return Err(ApiError::no_encontrado("Tarjeta", referencia));
        };
        tx.commit().await.map_err(error_db)?;
        self.canjear(&empresa.to_string(), dto, comprador_id).await
    }

    /// Expira los puntos de tarjetas activas sin movimientos durante
    /// `expira_meses`. Devuelve cuántas tarjetas se expiraron.
    ///
    /// Es idempotente: una tarjeta con saldo 0 se omite, y la expiración se
    /// registra como movimiento `expiracion` para conservar el historial.
    pub async fn expirar_inactivas(&self) -> Result<u64, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT t.id, t.empresa_id, t.puntos
               FROM orders.fidelizacion_tarjetas t
               JOIN orders.fidelizacion_programas p ON p.id = t.programa_id
              WHERE p.activo
                AND p.expira_meses IS NOT NULL
                AND t.estado = 'activa'
                AND t.puntos > 0
                AND COALESCE(
                      (SELECT MAX(m.creado_en)
                         FROM orders.fidelizacion_movimientos m
                        WHERE m.tarjeta_id = t.id),
                      t.creado_en
                    ) < NOW() - make_interval(months => p.expira_meses)
              FOR UPDATE OF t",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        let mut expiradas = 0_u64;
        for fila in &filas {
            let tarjeta_id = uuid_texto(fila, "id").map_err(error_db)?;
            let empresa_id = uuid_texto(fila, "empresa_id").map_err(error_db)?;
            let puntos: i32 = fila.try_get("puntos").map_err(error_db)?;
            sqlx::query(
                "INSERT INTO orders.fidelizacion_movimientos
                   (empresa_id, tarjeta_id, tipo, puntos, saldo_tras, descripcion)
                 VALUES ($1::uuid, $2::uuid, 'expiracion', $3, 0,
                         'Puntos expirados por inactividad')",
            )
            .bind(&empresa_id)
            .bind(&tarjeta_id)
            .bind(-puntos)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
            sqlx::query(
                "UPDATE orders.fidelizacion_tarjetas
                    SET puntos = 0, actualizado_en = NOW()
                  WHERE id = $1::uuid",
            )
            .bind(&tarjeta_id)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
            expiradas += 1;
        }
        tx.commit().await.map_err(error_db)?;
        Ok(expiradas)
    }
}

fn fila_programa(fila: &sqlx::postgres::PgRow) -> Result<Programa, ApiError> {
    Ok(Programa {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        nombre: fila.try_get("nombre").map_err(error_db)?,
        activo: fila.try_get("activo").map_err(error_db)?,
        puntos_por_unidad: fila.try_get("puntos_por_unidad").map_err(error_db)?,
        expira_meses: fila.try_get("expira_meses").map_err(error_db)?,
        config: jsonb(fila, "config").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn fila_tarjeta(fila: &sqlx::postgres::PgRow) -> Result<Tarjeta, ApiError> {
    Ok(Tarjeta {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        programa_id: uuid_texto(fila, "programa_id").map_err(error_db)?,
        parte_id: uuid_texto(fila, "parte_id").map_err(error_db)?,
        parte_nombre: fila.try_get("parte_nombre").map_err(error_db)?,
        codigo: fila.try_get("codigo").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn fila_movimiento(fila: &sqlx::postgres::PgRow) -> Result<Movimiento, ApiError> {
    Ok(Movimiento {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        tarjeta_id: uuid_texto(fila, "tarjeta_id").map_err(error_db)?,
        tipo: fila.try_get("tipo").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        saldo_tras: fila.try_get("saldo_tras").map_err(error_db)?,
        order_id: uuid_opcional(fila, "order_id").map_err(error_db)?,
        event_id: uuid_opcional(fila, "event_id").map_err(error_db)?,
        descripcion: fila.try_get("descripcion").map_err(error_db)?,
        creado_por: uuid_opcional(fila, "creado_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
    })
}

fn fila_recompensa(fila: &sqlx::postgres::PgRow) -> Result<Recompensa, ApiError> {
    Ok(Recompensa {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        codigo: fila.try_get("codigo").map_err(error_db)?,
        nombre: fila.try_get("nombre").map_err(error_db)?,
        descripcion: fila.try_get("descripcion").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        stock: fila.try_get("stock").map_err(error_db)?,
        activo: fila.try_get("activo").map_err(error_db)?,
        config: jsonb(fila, "config").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn fila_canje(fila: &sqlx::postgres::PgRow) -> Result<Canje, ApiError> {
    Ok(Canje {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        tarjeta_id: uuid_texto(fila, "tarjeta_id").map_err(error_db)?,
        recompensa_id: uuid_texto(fila, "recompensa_id").map_err(error_db)?,
        movimiento_id: uuid_texto(fila, "movimiento_id").map_err(error_db)?,
        order_id: uuid_opcional(fila, "order_id").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        creado_por: uuid_opcional(fila, "creado_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
    })
}

fn texto(dto: &Value, campo: &str) -> String {
    dto.get(campo)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn entero(dto: &Value, campo: &str) -> Option<i32> {
    dto.get(campo)
        .and_then(Value::as_i64)
        .and_then(|valor| i32::try_from(valor).ok())
}

fn codigo_valido(codigo: &str) -> bool {
    let bytes = codigo.as_bytes();
    (4..=32).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn codigo_recompensa_valido(codigo: &str) -> bool {
    let bytes = codigo.as_bytes();
    (2..=40).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
        })
}

fn es_unico(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|codigo| codigo == "23505")
}

fn error_db(error: sqlx::Error) -> ApiError {
    crate::event_store::error_db(error)
}
